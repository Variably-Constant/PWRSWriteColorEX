# A transcript records each line Write-ColorEX writes once, with no blank line after it, as
# PSWriteColorEX does (PSWriteColorEX issue #2). PowerShell records each Write-Host call as
# a line of its own, so Windows PowerShell 5.1, where each console color is a call of its own,
# records a line of several colors as one line per color.

BeforeDiscovery {
    $composesLines = $PSVersionTable.PSVersion -ge [version]'7.2' -and $Host.UI.SupportsVirtualTerminal
    $isDesktop = $PSVersionTable.PSEdition -eq 'Desktop'
}

BeforeAll {
    . (Join-Path $PSScriptRoot 'Common.ps1')
    $saved = Save-ColorEnvironment
    Set-ColorEnvironment @{}

    # The lines a transcript recorded between its header and its footer
    function Get-TranscriptBody {
        param([string]$Path)
        $lines = @(Get-Content -LiteralPath $Path)
        $stars = @(for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i] -match '^\*{22}$') { $i } })
        if ($stars[2] - $stars[1] -le 1) { return @() }
        return @($lines[($stars[1] + 1)..($stars[2] - 1)])
    }

}

AfterAll {
    Restore-ColorEnvironment $saved
    Import-PWRSWriteColorEX
}

Describe 'Write-ColorEX in a transcript' -Tag 'Transcript' {
    It 'records a line of one color as one line, with no blank line after it' {
        $path = Join-Path $TestDrive 'one-color.txt'
        $null = Start-Transcript -Path $path
        try {
            Write-ColorEX -Text 'first' -Color Green
            Write-ColorEX -Text 'second'
            Write-ColorEX -Text 'third' -LinesBefore 2
        } finally {
            $null = Stop-Transcript
        }

        $body = Get-TranscriptBody -Path $path | ForEach-Object { Remove-EscapeCode $_ }
        $body | Should -Be @('first', 'second', '', '', 'third')
    }

    It 'records a line of several colors as one line' -Skip:(-not $composesLines) {
        $path = Join-Path $TestDrive 'several-colors.txt'
        $null = Start-Transcript -Path $path
        try {
            Write-ColorEX -Text ' - Consumer', '- Count: ', '55' -Color White, Gray, Cyan
            Write-ColorEX -Text '   - Sum: ', '1000' -Color Gray, Green
            Write-ColorEX -Text 'indented' -StartSpaces 4 -Color Yellow
        } finally {
            $null = Stop-Transcript
        }

        Get-TranscriptBody -Path $path | Should -Be @(' - Consumer- Count: 55', '   - Sum: 1000', '    indented')
    }

    It 'records each color as a line of its own, with no escape codes, in Windows PowerShell 5.1' -Skip:(-not $isDesktop) {
        $path = Join-Path $TestDrive 'desktop.txt'
        $null = Start-Transcript -Path $path
        try {
            Write-ColorEX -Text 'a', 'b' -Color Red, Green
        } finally {
            $null = Stop-Transcript
        }

        Get-TranscriptBody -Path $path | Should -Be @('a', 'b')
    }
}
