# Runs the cases in parity/cases.txt against PSWriteColorEX and against this module, each in a
# process of its own, under each color environment, and compares every write to the host with its
# colors, every warning, error and object, and the bytes of every log file; then compares the files
# this module copies from PSWriteColorEX, the theme files and the help topic, with PSWriteColorEX's.
# PSWRITECOLOREX names the PSWriteColorEX manifest to compare with. A run without it cannot say
# whether this module writes what PSWriteColorEX writes, so it fails rather than passing on nothing.

BeforeDiscovery {
    $script:environments = @(
        @{ Name = 'FORCE_COLOR=3 (TrueColor)'; Values = @{ FORCE_COLOR = '3' } }
        @{ Name = 'FORCE_COLOR=2 (ANSI8)'; Values = @{ FORCE_COLOR = '2' } }
        @{ Name = 'FORCE_COLOR=1 (ANSI4)'; Values = @{ FORCE_COLOR = '1' } }
        @{ Name = 'NO_COLOR=1'; Values = @{ NO_COLOR = '1' } }
        @{ Name = 'TERM=dumb'; Values = @{ TERM = 'dumb' } }
        @{ Name = 'CLICOLOR=0'; Values = @{ CLICOLOR = '0' } }
        @{ Name = 'CLICOLOR_FORCE=1 and TERM=dumb'; Values = @{ CLICOLOR_FORCE = '1'; TERM = 'dumb' } }
        @{ Name = 'the support detected'; Values = @{} }
    )
}

BeforeAll {
    . (Join-Path $PSScriptRoot 'Common.ps1')

    if (-not $env:PSWRITECOLOREX) {
        throw 'PSWRITECOLOREX is not set; it names the PSWriteColorEX manifest the parity suite compares with: check out https://github.com/MarkusMcNugen/PSWriteColorEX and set PSWRITECOLOREX to its PSWriteColorEX.psd1'
    }
    if (-not (Test-Path -LiteralPath $env:PSWRITECOLOREX -PathType Leaf)) {
        throw "PSWRITECOLOREX names $env:PSWRITECOLOREX, which is not a file"
    }
    if (-not $env:PWRS_MODULE) {
        throw 'PWRS_MODULE is not set; run the suites through cargo pwrs test'
    }

    $script:runner = Join-Path $PSScriptRoot 'parity/Invoke-Cases.ps1'
    $script:cases = Join-Path $PSScriptRoot 'parity/cases.txt'
    $script:caseCount = @(Get-Content -LiteralPath $script:cases -Encoding UTF8 | Where-Object { $_ -and $_ -notmatch '^\s*#' }).Count
    $script:modules = @{
        PSWriteColorEX = $env:PSWRITECOLOREX
        PWRSWriteColorEX = Join-Path $env:PWRS_MODULE 'PWRSWriteColorEX.psd1'
    }
    # The host running this suite, pwsh or Windows PowerShell, runs the cases too
    $script:shell = (Get-Process -Id $PID).Path

    # The records of every case, run with the module under the color variables given
    function Invoke-ParityCase {
        param([string]$Module, [hashtable]$Values, [string]$Name)
        $saved = Save-ColorEnvironment
        try {
            foreach ($variable in $script:colorVariables) {
                [System.Environment]::SetEnvironmentVariable($variable, $null)
            }
            [System.Environment]::SetEnvironmentVariable('TERM', 'xterm-256color')
            foreach ($variable in $Values.Keys) {
                [System.Environment]::SetEnvironmentVariable($variable, $Values[$variable])
            }
            $out = Join-Path $TestDrive "$Name.clixml"
            $folder = Join-Path $TestDrive "$Name-files"
            & $script:shell -NoProfile -NonInteractive -File $script:runner -ModulePath $script:modules[$Module] -CasesFile $script:cases -OutFile $out -TestDir $folder | Out-Null
            if (-not (Test-Path -LiteralPath $out)) {
                throw "The cases did not run against $Module"
            }
            Import-Clixml -LiteralPath $out
        } finally {
            Restore-ColorEnvironment $saved
        }
    }
}

Describe 'Parity with PSWriteColorEX' -Tag 'Parity' {
    It 'writes what PSWriteColorEX writes with <Name>' -TestCases $script:environments {
        $id = ($Name -replace '[^A-Za-z0-9]', '')
        $expected = Invoke-ParityCase -Module PSWriteColorEX -Values $Values -Name "ps-$id"
        $actual = Invoke-ParityCase -Module PWRSWriteColorEX -Values $Values -Name "pwrs-$id"

        $expected = @($expected)
        $actual = @($actual)
        $expected.Count | Should -Be $script:caseCount
        $actual.Count | Should -Be $script:caseCount
        $differences = for ($i = 0; $i -lt $expected.Count; $i++) {
            $want = @($expected[$i].Records) -join "`n"
            $got = @($actual[$i].Records) -join "`n"
            if ($want -cne $got) {
                "$($expected[$i].Case)`n    PSWriteColorEX:   " + ($want -replace "`n", "`n                      ") + "`n    PWRSWriteColorEX: " + ($got -replace "`n", "`n                      ")
            }
        }
        $differences -join "`n" | Should -BeNullOrEmpty
    }
}

Describe 'Files copied from PSWriteColorEX' -Tag 'Parity' {
    BeforeAll {
        $script:theirRoot = Split-Path -Parent $env:PSWRITECOLOREX
        $script:ourRoot = Split-Path -Parent $PSScriptRoot
    }

    It 'has every theme file of PSWriteColorEX, byte for byte' {
        $theirThemes = Join-Path $script:theirRoot 'Themes'
        $ourThemes = Join-Path $script:ourRoot 'themes'
        $theirNames = [string[]]@(Get-ChildItem -LiteralPath $theirThemes -Filter '*.json' -File | ForEach-Object Name)
        $ourNames = [string[]]@(Get-ChildItem -LiteralPath $ourThemes -Filter '*.json' -File | ForEach-Object Name)
        [Array]::Sort($theirNames, [StringComparer]::Ordinal)
        [Array]::Sort($ourNames, [StringComparer]::Ordinal)
        $theirNames.Count | Should -BeGreaterThan 0
        $ourNames | Should -Be $theirNames
        foreach ($name in $theirNames) {
            [Convert]::ToBase64String([IO.File]::ReadAllBytes((Join-Path $ourThemes $name))) |
                Should -BeExactly ([Convert]::ToBase64String([IO.File]::ReadAllBytes((Join-Path $theirThemes $name)))) -Because "themes/$name is a copy of PSWriteColorEX's"
        }
    }

    It 'has PSWriteColorEX''s help topic under its own name' {
        $ourPath = Join-Path $script:ourRoot 'en-US/about_PWRSWriteColorEX.help.txt'
        # The topic's examples hold characters outside ASCII, which Windows PowerShell reads only
        # after a byte order mark
        [IO.File]::ReadAllBytes($ourPath)[0..2] | Should -Be @(0xEF, 0xBB, 0xBF)
        $theirs = [IO.File]::ReadAllText((Join-Path $script:theirRoot 'en-US/about_PSWriteColorEX.help.txt'))
        $ours = [IO.File]::ReadAllText($ourPath)
        $marker = "`nLONG DESCRIPTION`n"
        $theirs.IndexOf($marker) | Should -BeGreaterThan 0
        $ours.IndexOf($marker) | Should -BeGreaterThan 0
        # From the long description on, the same text
        $ours.Substring($ours.IndexOf($marker)) | Should -BeExactly $theirs.Substring($theirs.IndexOf($marker))
        # Before it, the same words with this module's name, within 80 columns
        $ourHead = $ours.Substring(0, $ours.IndexOf($marker))
        $theirHead = $theirs.Substring(0, $theirs.IndexOf($marker)) -creplace 'PSWriteColorEX', 'PWRSWriteColorEX'
        ($ourHead -split '\s+') -join ' ' | Should -BeExactly (($theirHead -split '\s+') -join ' ')
        foreach ($line in $ourHead -split "`n") {
            $line.Length | Should -BeLessOrEqual 80 -Because $line
        }
    }
}
