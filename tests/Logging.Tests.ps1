# The log file -LogFile writes: where it goes, what each line holds, and its bytes in each encoding.

BeforeDiscovery {
    $encodingCases = @(
        @{ Encoding = 'utf8'; Bytes = 'C3-A9' }
        @{ Encoding = 'utf8NoBOM'; Bytes = 'C3-A9' }
        @{ Encoding = 'default'; Bytes = 'C3-A9' }
        @{ Encoding = 'utf8BOM'; Bytes = 'EF-BB-BF-C3-A9' }
        @{ Encoding = 'unicode'; Bytes = 'FF-FE-E9-00' }
        @{ Encoding = 'string'; Bytes = 'FF-FE-E9-00' }
        @{ Encoding = 'bigendianunicode'; Bytes = 'FE-FF-00-E9' }
        @{ Encoding = 'utf32'; Bytes = 'FF-FE-00-00-E9-00-00-00' }
        @{ Encoding = 'bigendianutf32'; Bytes = '00-00-FE-FF-00-00-00-E9' }
        @{ Encoding = 'ascii'; Bytes = '3F' }
    )
}

BeforeAll {
    . (Join-Path $PSScriptRoot 'Common.ps1')
    Import-PWRSWriteColorEX
    $eAcute = [string][char]0xE9
    $newLine = [System.Environment]::NewLine
}

Describe 'Write-ColorEX -LogFile' -Tag 'Logging' {
    BeforeEach {
        $folder = Join-Path $TestDrive ([guid]::NewGuid().ToString('N'))
        $null = New-Item -ItemType Directory -Path $folder
    }

    It 'writes <Encoding> as <Bytes>' -TestCases $encodingCases {
        Write-ColorEX -Text $eAcute -LogFile 'e.log' -LogPath $folder -Encoding $Encoding -NoNewLine -NoConsoleOutput
        [BitConverter]::ToString([IO.File]::ReadAllBytes((Join-Path $folder 'e.log'))) | Should -Be $Bytes
    }

    It 'writes a byte order mark only into a new file' {
        Write-ColorEX -Text 'a' -LogFile 'bom.log' -LogPath $folder -Encoding utf8BOM -NoConsoleOutput
        Write-ColorEX -Text 'b' -LogFile 'bom.log' -LogPath $folder -Encoding utf8BOM -NoConsoleOutput
        $bytes = [IO.File]::ReadAllBytes((Join-Path $folder 'bom.log'))
        $text = [Text.Encoding]::UTF8.GetString($bytes, 3, $bytes.Length - 3)
        [BitConverter]::ToString($bytes, 0, 3) | Should -Be 'EF-BB-BF'
        $text | Should -Be "a$($newLine)b$newLine"
    }

    It 'writes the segments as one line, with the time and level before them' {
        Write-ColorEX -Text 'a', 'b' -LogFile 'line.log' -LogPath $folder -LogTime -DateTimeFormat "'T'" -LogLevel 'INFO' -NoConsoleOutput
        [IO.File]::ReadAllText((Join-Path $folder 'line.log')) | Should -Be "[T][INFO] ab$newLine"
    }

    It 'adds .log to a name without an extension' {
        Write-ColorEX -Text 'x' -LogFile 'plain' -LogPath $folder -NoConsoleOutput
        Join-Path $folder 'plain.log' | Should -Exist
    }

    It 'creates the folder of the log file' {
        $path = Join-Path $folder 'a/b/deep.log'
        Write-ColorEX -Text 'x' -LogFile $path -NoConsoleOutput
        $path | Should -Exist
    }

    It 'puts a bare name in the folder of the calling script' {
        $script = Join-Path $folder 'caller.ps1'
        Set-Content -LiteralPath $script -Value "Write-ColorEX -Text 'x' -LogFile 'from-script.log' -NoConsoleOutput; Write-ColorInfo 'y' -LogFile 'from-helper.log' -NoConsoleOutput"
        & $script
        Join-Path $folder 'from-script.log' | Should -Exist
        Join-Path $folder 'from-helper.log' | Should -Exist
    }

    It 'puts a bare name in the current location when called from the prompt' {
        Push-Location -LiteralPath $folder
        try {
            & ([scriptblock]::Create("Write-ColorEX -Text 'x' -LogFile 'here.log' -NoConsoleOutput"))
        } finally {
            Pop-Location
        }
        Join-Path $folder 'here.log' | Should -Exist
    }

    It 'warns after the last try when the file cannot be written' {
        $blocker = Join-Path $folder 'blocker'
        Set-Content -LiteralPath $blocker -Value ''
        $result = Invoke-Captured { Write-ColorEX -Text 'x' -LogFile (Join-Path $blocker 'x.log') -LogRetry 2 -NoConsoleOutput }
        $result.Warnings.Count | Should -Be 1
        $result.Warnings[0] | Should -BeLike "Write-ColorEX - Couldn't write to log file *. Tried (2/2)"
    }

    It 'logs the padding -AutoPad adds' {
        Write-ColorEX -Text 'ab' -AutoPad 5 -LogFile 'pad.log' -LogPath $folder -NoConsoleOutput
        [IO.File]::ReadAllText((Join-Path $folder 'pad.log')) | Should -Be "ab   $newLine"
    }
}
