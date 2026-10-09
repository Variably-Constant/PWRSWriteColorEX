# What Write-ColorEX writes to the host. FORCE_COLOR sets the color mode, so the ANSI cases write
# the same escape codes on every host. A line of console colors alone is one call with escape
# codes where PowerShell 7.2 or later runs in a host that shows them, and one call per color
# elsewhere, as Write-Host writes it.

BeforeDiscovery {
    $composes = $PSVersionTable.PSVersion -ge [version]'7.2' -and $Host.UI.SupportsVirtualTerminal
}

BeforeAll {
    . (Join-Path $PSScriptRoot 'Common.ps1')
    $saved = Save-ColorEnvironment
}

AfterAll {
    Restore-ColorEnvironment $saved
    Import-PWRSWriteColorEX
}

Describe 'Write-ColorEX in TrueColor' -Tag 'Output' {
    BeforeAll {
        Set-ColorEnvironment @{ FORCE_COLOR = '3' }
    }

    It 'writes a hex code without -TrueColor as TrueColor' {
        Get-HostText { Write-ColorEX -Text 'x' -Color '#FF8000' } | Should -Be "$esc[38;2;255;128;0mx$esc[0m"
    }

    It 'writes a color name as its RGB value with -TrueColor' {
        Get-HostText { Write-ColorEX -Text 'x' -Color Orange -TrueColor } | Should -Be "$esc[38;2;255;165;0mx$esc[0m"
    }

    It 'reads three integers for one segment as one RGB color' {
        Get-HostText { Write-ColorEX -Text 'x' -Color 255, 0, 128 -TrueColor } | Should -Be "$esc[38;2;255;0;128mx$esc[0m"
    }

    It 'clamps RGB values to 0-255 with a warning' {
        $result = Invoke-Captured { Write-ColorEX -Text 'x' -Color @(, @(300, -5, 1)) -TrueColor }
        $result.Text | Should -Be "$esc[38;2;255;0;1mx$esc[0m"
        $result.Warnings | Should -Be 'RGB values out of range (0-255). Original: @(300,-5,1). Clamped to: @(255,0,1)'
    }

    It 'leaves out the warnings with -Silent' {
        (Invoke-Captured { Write-ColorEX -Text 'x' -Color @(, @(300, 0, 0)) -TrueColor -Silent }).Warnings | Should -BeNullOrEmpty
    }

    It 'writes an invalid hex code as gray with a warning' {
        $result = Invoke-Captured { Write-ColorEX -Text 'x' -Color '#GGGGGG' }
        $result.Text | Should -Be "$esc[38;2;128;128;128mx$esc[0m"
        $result.Warnings | Should -Be 'Invalid hex color format: GGGGGG. Expected format: #RRGGBB or RRGGBB'
    }

    It 'writes a background color' {
        Get-HostText { Write-ColorEX -Text 'x' -BackGroundColor '#102030' } | Should -Be "$esc[48;2;16;32;48mx$esc[0m"
    }

    It 'writes a gradient across the characters, blended in OKLab' {
        Get-HostText { Write-ColorEX -Text 'abc' -Gradient '#FF0000', '#0000FF' } |
            Should -Be "$esc[38;2;255;0;0ma$esc[38;2;140;83;162mb$esc[38;2;0;0;255mc$esc[0m"
    }

    It 'blends each channel on its own with -GradientSpace RGB' {
        Get-HostText { Write-ColorEX -Text 'abc' -Gradient '#FF0000', '#0000FF' -GradientSpace RGB } |
            Should -Be "$esc[38;2;255;0;0ma$esc[38;2;128;0;128mb$esc[38;2;0;0;255mc$esc[0m"
    }

    It 'gives an emoji one gradient step and keeps it whole' {
        $emoji = [char]::ConvertFromUtf32(0x1F600)
        $text = Get-HostText { Write-ColorEX -Text "a$($emoji)b" -Gradient '#FF0000', '#0000FF' }
        $text | Should -Be "$esc[38;2;255;0;0ma$esc[38;2;140;83;162m$emoji$esc[38;2;0;0;255mb$esc[0m"
    }

    It 'gives an emoji joined by U+200D one gradient step' {
        $family = -join (@(0x1F468, 0x200D, 0x1F469, 0x200D, 0x1F467) | ForEach-Object { [char]::ConvertFromUtf32($_) })
        $text = Get-HostText { Write-ColorEX -Text "$($family)x" -Gradient '#FF0000', '#0000FF' }
        $text | Should -Be "$esc[38;2;255;0;0m$family$esc[38;2;0;0;255mx$esc[0m"
    }

    It 'writes a segment with its own color in that color instead of the gradient' {
        Get-HostText { Write-ColorEX -Text 'ab' -Gradient '#FF0000', '#0000FF' -Color '#00FF00' } | Should -Be "$esc[38;2;0;255;0mab$esc[0m"
    }

    It 'leaves a segment with a $null color to the gradient' {
        $text = Get-HostText { Write-ColorEX -Text 'ab', 'CD', 'ef' -Gradient '#FF0000', '#0000FF' -Color $null, Yellow, $null }
        $text | Should -Match ([regex]::Escape("$esc[38;2;255;255;0mCD$esc[0m"))
        $text | Should -Match ([regex]::Escape("$esc[38;2;255;0;0ma"))
        ([regex]::Matches($text, [regex]::Escape("$esc[38;2;"))).Count | Should -Be 5
        Remove-EscapeCode $text | Should -Be 'abCDef'
    }

    It 'takes $null for the whole of -Color as no color' {
        Get-HostText { Write-ColorEX -Text 'x' -Color $null } | Should -Be 'x'
    }

    It 'refuses an entry that is not a color, naming the parameter and the value' {
        { Write-ColorEX -Text 'x' -Color 1.5 -ErrorAction Stop } |
            Should -Throw "Cannot validate argument on parameter 'Color'. The argument `"1.5`" is not a color*"
        { Write-ColorEX -Text 'x' -BackGroundColor @{ Red = 1 } -ErrorAction Stop } |
            Should -Throw "*parameter 'BackGroundColor'. The argument `"System.Collections.Hashtable`" is not a color*"
    }

    It 'turns a gradient of one color off with a warning' {
        $result = Invoke-Captured { Write-ColorEX -Text 'ab' -Gradient Red }
        $result.Text | Should -Be 'ab'
        $result.Warnings | Should -Be 'Gradient requires at least 2 colors (received 1). Gradient disabled.'
    }
}

Describe 'Write-ColorEX in 256 and 16 colors' -Tag 'Output' {
    It 'writes an ANSI8 number in 256 colors' {
        Set-ColorEnvironment @{ FORCE_COLOR = '2' }
        Get-HostText { Write-ColorEX -Text 'x' -Color 208 -ANSI8 } | Should -Be "$esc[38;5;208mx$esc[0m"
    }

    It 'falls back from TrueColor to 256 colors with a warning' {
        Set-ColorEnvironment @{ FORCE_COLOR = '2' }
        $result = Invoke-Captured { Write-ColorEX -Text 'x' -Color '#FF8000' -TrueColor }
        $result.Text | Should -Be "$esc[38;5;208mx$esc[0m"
        $result.Warnings | Should -Be 'TrueColor not supported by terminal. Falling back to ANSI8 (256 colors).'
    }

    It 'writes a hex code without a color mode in 256 colors without a warning' {
        Set-ColorEnvironment @{ FORCE_COLOR = '2' }
        $result = Invoke-Captured { Write-ColorEX -Text 'x' -Color '#FF8000' }
        $result.Text | Should -Be "$esc[38;5;208mx$esc[0m"
        $result.Warnings | Should -BeNullOrEmpty
    }

    It 'writes an ANSI8 number as the nearest of 16 colors' {
        Set-ColorEnvironment @{ FORCE_COLOR = '1' }
        Get-HostText { Write-ColorEX -Text 'x' -Color 208 -ANSI8 -Silent } | Should -Be "$esc[93mx$esc[0m"
        Get-HostText { Write-ColorEX -Text 'x' -BackGroundColor 208 -ANSI8 -Silent } | Should -Be "$esc[103mx$esc[0m"
    }

    It 'writes the gray steps of a 256-color gradient' {
        Set-ColorEnvironment @{ FORCE_COLOR = '2' }
        Get-HostText { Write-ColorEX -Text 'abc' -Gradient '#202020', '#E0E0E0' -ANSI8 -GradientSpace RGB } |
            Should -Be "$esc[38;5;234ma$esc[38;5;244mb$esc[38;5;254mc$esc[0m"
        Get-HostText { Write-ColorEX -Text 'abc' -Gradient '#202020', '#E0E0E0' -ANSI8 } |
            Should -Be "$esc[38;5;234ma$esc[38;5;243mb$esc[38;5;254mc$esc[0m"
    }

    It 'turns a gradient off with a warning in 16 colors' {
        Set-ColorEnvironment @{ FORCE_COLOR = '1' }
        $result = Invoke-Captured { Write-ColorEX -Text 'ab' -Gradient Red, Blue }
        $result.Text | Should -Be 'ab'
        $result.Warnings | Should -Be 'Gradient requires ANSI 256-color or TrueColor support. Terminal supports: ANSI4 (16 colors). Gradient disabled.'
    }
}

Describe 'Write-ColorEX styles' -Tag 'Output' {
    BeforeAll {
        Set-ColorEnvironment @{ FORCE_COLOR = '1' }
    }

    It 'styles every segment with the style switches' {
        Get-HostText { Write-ColorEX -Text 'x', 'y' -Italic } | Should -Be "$esc[3mx$esc[0m$esc[3my$esc[0m"
    }

    It 'styles the first segment with one style given alone' {
        Get-HostText { Write-ColorEX -Text 'x', 'y' -Style 'Bold' } | Should -Be "$esc[1mx$esc[0my"
    }

    It 'styles each segment with its entry in -Style' {
        Get-HostText { Write-ColorEX -Text 'x', 'y' -Style @(@('Bold', 'Underline'), 'Italic') } |
            Should -Be "$esc[1m$esc[4mx$esc[0m$esc[3my$esc[0m"
    }

    It 'writes each style code' {
        Get-HostText { Write-ColorEX -Text 'x' -Bold -Faint -Italic -Underline -Blink -CrossedOut -DoubleUnderline -Overline } |
            Should -Be "$esc[1m$esc[2m$esc[3m$esc[4m$esc[5m$esc[9m$esc[21m$esc[53mx$esc[0m"
    }
}

Describe 'Write-ColorEX lines and layout' -Tag 'Output' {
    BeforeAll {
        Set-ColorEnvironment @{ NO_COLOR = '1' }
    }

    It 'writes the segments of a line in one call' {
        $writes = @(Get-HostWrite { Write-ColorEX -Text 'a', 'b', 'c' -Color Red, Green })
        $writes.Count | Should -Be 1
        $writes[0].Text | Should -Be 'abc'
        $writes[0].NoNewLine | Should -BeFalse
    }

    It 'writes a blank line for each of -LinesBefore and -LinesAfter' {
        $writes = @(Get-HostWrite { Write-ColorEX -Text 'x' -LinesBefore 2 -LinesAfter 1 })
        @($writes | ForEach-Object Text) | Should -Be @('', '', 'x', '')
    }

    It 'indents with tabs, then spaces' {
        Get-HostText { Write-ColorEX -Text 'x' -StartTab 2 -StartSpaces 3 } | Should -Be "`t`t   x"
    }

    It 'writes the time in the format given' {
        Get-HostText { Write-ColorEX -Text 'x' -ShowTime -DateTimeFormat "'T'" } | Should -Be '[T] x'
    }

    It 'leaves the line open with -NoNewLine' {
        (Get-HostWrite { Write-ColorEX -Text 'x' -NoNewLine }).NoNewLine | Should -BeTrue
    }

    It 'writes nothing to the host with -NoConsoleOutput' {
        Get-HostWrite { Write-ColorEX -Text 'x' -NoConsoleOutput } | Should -BeNullOrEmpty
    }

    It 'pads to a display width, counting wide characters as 2 cells' {
        Get-HostText { Write-ColorEX -Text "$([char]0x4E16)$([char]0x754C)" -AutoPad 6 } | Should -Be "$([char]0x4E16)$([char]0x754C)  "
    }

    It 'pads on the left with -PadLeft and the character given' {
        Get-HostText { Write-ColorEX -Text 'abc' -AutoPad 6 -PadLeft -PadChar '.' } | Should -Be '...abc'
    }

    It 'writes each piped string as a line of its own' {
        Get-HostText { 'one', 'two' | Write-ColorEX -Color Green } | Should -Be @('one', 'two')
    }

    It 'writes an empty line for -BlankLine with output redirected' {
        @(Get-HostWrite { Write-ColorEX -BlankLine }).Count | Should -Be 1
    }
}

Describe 'Write-ColorEX colors turned off' -Tag 'Output' {
    It 'writes plain text with <Name>' -TestCases @(
        @{ Name = 'NO_COLOR'; Values = @{ NO_COLOR = '1' } }
        @{ Name = 'FORCE_COLOR=0'; Values = @{ FORCE_COLOR = '0' } }
        @{ Name = 'TERM=dumb'; Values = @{ TERM = 'dumb' } }
    ) {
        Set-ColorEnvironment $Values
        $writes = @(Get-HostWrite { Write-ColorEX -Text 'a', 'b' -Color Red, '#00FF00' -Bold -Gradient Red, Blue })
        $writes.Count | Should -Be 1
        $writes[0].Text | Should -Be 'ab'
    }
}

Describe 'Write-ColorEX console colors' -Tag 'Output' {
    BeforeAll {
        Set-ColorEnvironment @{}
    }

    It 'writes a line of console colors in one call with escape codes' -Skip:(-not $composes) {
        $writes = @(Get-HostWrite { Write-ColorEX -Text 'a', 'b' -Color Red, Green })
        $writes.Count | Should -Be 1
        $writes[0].Text | Should -Be "$esc[91ma$esc[0m$esc[92mb$esc[0m"
    }

    It 'writes one call per color with the line end on the last' -Skip:$composes {
        $writes = @(Get-HostWrite { Write-ColorEX -Text 'a', 'b' -Color Red, Green })
        @($writes | ForEach-Object Text) | Should -Be @('a', 'b')
        @($writes | ForEach-Object { "$($_.ForegroundColor)" }) | Should -Be @('Red', 'Green')
        @($writes | ForEach-Object NoNewLine) | Should -Be @($true, $false)
    }
}

Describe 'Write-ColorEX style profiles' -Tag 'Output' {
    BeforeAll {
        Set-ColorEnvironment @{ FORCE_COLOR = '3' }
    }

    It 'applies a style profile' {
        Get-HostText { Write-ColorEX -Text 'x' -StyleProfile ([PSColorStyle]::Profiles['Error']) } | Should -Be "$esc[1m$esc[91mx$esc[0m"
    }

    It 'keeps a parameter given over the profile' {
        Get-HostText { Write-ColorEX -Text 'x' -StyleProfile ([PSColorStyle]::Profiles['Error']) -Color Blue } | Should -Be "$esc[1m$esc[94mx$esc[0m"
    }

    It 'applies a style made with New-ColorStyle, its hex color in TrueColor' {
        $style = New-ColorStyle -Name 'Accent' -ForegroundColor '#FF6B35' -StartTab 1
        Get-HostText { Write-ColorEX -Text 'x' -StyleProfile $style } | Should -Be "`t$esc[38;2;255;107;53mx$esc[0m"
    }

    It 'applies the default style with -Default' {
        try {
            Set-ColorDefault -ForegroundColor Cyan -Italic
            Get-HostText { Write-ColorEX -Text 'x' -Default } | Should -Be "$esc[3m$esc[96mx$esc[0m"
        } finally {
            [PSColorStyle]::InitializeDefaultProfiles()
        }
    }
}
