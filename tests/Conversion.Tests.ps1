# The color conversions, the color table, Measure-DisplayWidth and Test-AnsiSupport.

BeforeDiscovery {
    # Each case: a name, the code points of the text, its width, and its width with -AmbiguousAsWide
    $widthCases = @(
        @{ Name = 'printable ASCII'; CodePoints = @(0x48, 0x65, 0x6C, 0x6C, 0x6F); Width = 5; WideWidth = 5 }
        @{ Name = 'CJK characters'; CodePoints = @(0x4E16, 0x754C); Width = 4; WideWidth = 4 }
        @{ Name = 'check mark U+2713'; CodePoints = @(0x2713); Width = 1; WideWidth = 1 }
        @{ Name = 'check mark button U+2705'; CodePoints = @(0x2705); Width = 2; WideWidth = 2 }
        @{ Name = 'two emoji'; CodePoints = @(0x1F600, 0x1F44D); Width = 4; WideWidth = 4 }
        @{ Name = 'black circle (ambiguous)'; CodePoints = @(0x25CF); Width = 1; WideWidth = 2 }
        @{ Name = 'box drawing (ambiguous)'; CodePoints = @(0x2554, 0x2550, 0x2550, 0x2550, 0x2557); Width = 5; WideWidth = 10 }
        @{ Name = 'e and a combining acute accent'; CodePoints = @(0x65, 0x301); Width = 1; WideWidth = 1 }
        @{ Name = 'zero width space'; CodePoints = @(0x200B); Width = 0; WideWidth = 0 }
        @{ Name = 'tab between letters'; CodePoints = @(0x61, 0x9, 0x62); Width = 2; WideWidth = 2 }
        @{ Name = 'warning sign with U+FE0F'; CodePoints = @(0x26A0, 0xFE0F); Width = 2; WideWidth = 2 }
        @{ Name = 'watch with U+FE0E'; CodePoints = @(0x231A, 0xFE0E); Width = 1; WideWidth = 2 }
        @{ Name = 'thumbs up with skin tone'; CodePoints = @(0x1F44D, 0x1F3FD); Width = 2; WideWidth = 2 }
        @{ Name = 'family joined by U+200D'; CodePoints = @(0x1F468, 0x200D, 0x1F469, 0x200D, 0x1F467); Width = 2; WideWidth = 2 }
        @{ Name = 'heart on fire'; CodePoints = @(0x2764, 0xFE0F, 0x200D, 0x1F525); Width = 2; WideWidth = 2 }
        @{ Name = 'flag'; CodePoints = @(0x1F1FA, 0x1F1F8); Width = 2; WideWidth = 2 }
    )
}

BeforeAll {
    . (Join-Path $PSScriptRoot 'Common.ps1')
    Import-PWRSWriteColorEX

    # Builds a string from code points, so this file holds only ASCII
    function ConvertTo-TestString {
        param([int[]]$CodePoints)
        -join ($CodePoints | ForEach-Object { [char]::ConvertFromUtf32($_) })
    }
}

Describe 'Color conversions' -Tag 'Conversion' {
    It 'converts a hex code to three integers' {
        $rgb = Convert-HexToRGB '#FF8000'
        $rgb | Should -Be @(255, 128, 0)
        $rgb[0] | Should -BeOfType [int]
    }

    It 'reads 0x in either case and no prefix' {
        Convert-HexToRGB '0XFF8000' | Should -Be @(255, 128, 0)
        Convert-HexToRGB 'ff8000' | Should -Be @(255, 128, 0)
    }

    It 'answers gray with a warning for an invalid code' {
        $rgb = Convert-HexToRGB 'nope' -WarningVariable warnings -WarningAction SilentlyContinue
        $rgb | Should -Be @(128, 128, 128)
        $warnings | Should -Be 'Invalid hex color format: nope. Expected format: #RRGGBB or RRGGBB'
    }

    It 'converts RGB to the nearest 256-color code' {
        Convert-RGBToANSI8 @(255, 128, 0) | Should -Be 208
        Convert-RGBToANSI8 @(128, 128, 128) | Should -Be 244
        Convert-RGBToANSI8 @(0, 0, 0) | Should -Be 16
        Convert-RGBToANSI8 @(255, 255, 255) | Should -Be 231
        Convert-RGBToANSI8 @(300, -20, 0) | Should -Be 196
        Convert-RGBToANSI8 @(128, 128, 128) | Should -BeOfType [int]
    }

    It 'converts RGB to the nearest 16-color code' {
        Convert-RGBToANSI4 @(255, 0, 0) | Should -Be 91
        Convert-RGBToANSI4 @(128, 0, 0) | Should -Be 31
        Convert-RGBToANSI4 @(255, 128, 0) | Should -Be 93
        Convert-RGBToANSI4 @(100, 100, 100) | Should -Be 90
    }

    It 'lightens RGB values, rounding halves to even' {
        Get-LighterRGBColor @(139, 0, 0) | Should -Be @(195, 102, 102)
        Get-LighterRGBColor @(50, 50, 50) -Factor 2 | Should -Be @(255, 255, 255)
    }

    It 'lightens a color name within its family' {
        Get-LighterColorName 'DarkRed' | Should -Be 'Red'
        Get-LighterColorName 'Red' | Should -Be 'LightRed'
        Get-LighterColorName 'White' | Should -Be 'White'
        Get-LighterColorName 'LightRed' | Should -Be 'LightRed'
    }

    It 'lightens a 256-color code' {
        Get-LighterANSI8Color 4 | Should -Be 12
        Get-LighterANSI8Color 240 | Should -Be 244
        Get-LighterANSI8Color 196 | Should -Be 203
        Get-LighterANSI8Color 100 -Factor 1.6 | Should -Be 186
    }

    It 'refuses a 256-color code outside 0-255' {
        { Get-LighterANSI8Color 300 -ErrorAction Stop } | Should -Throw
    }

    It 'answers to the Lighten-* names' {
        Lighten-ColorName 'DarkBlue' | Should -Be 'Blue'
        LA8 1 | Should -Be 9
    }
}

Describe 'Get-ColorTableWithRGB' -Tag 'Conversion' {
    BeforeAll {
        $table = Get-ColorTableWithRGB
    }

    It 'holds 129 color names' {
        $table | Should -BeOfType [hashtable]
        $table.Count | Should -Be 129
    }

    It 'finds a name without regard to case' {
        $table['ORANGE'][3] | Should -Be 208
    }

    It 'holds each color in every mode' {
        $entry = $table['DarkRed']
        $entry[0] | Should -Be 'DarkRed'
        $entry[1] | Should -Be 31
        $entry[2] | Should -Be 41
        $entry[3] | Should -Be 52
        $entry[4] | Should -Be @(139, 0, 0)
        $entry[1] | Should -BeOfType [int]
    }
}

Describe 'Measure-DisplayWidth' -Tag 'Conversion' {
    It 'measures <Name> as <Width> cells' -TestCases $widthCases {
        Measure-DisplayWidth -Text (ConvertTo-TestString $CodePoints) | Should -Be $Width
    }

    It 'measures <Name> as <WideWidth> cells with -AmbiguousAsWide' -TestCases $widthCases {
        Measure-DisplayWidth -Text (ConvertTo-TestString $CodePoints) -AmbiguousAsWide | Should -Be $WideWidth
    }

    It 'measures each string piped in' {
        'ab', 'abc' | Measure-DisplayWidth | Should -Be @(2, 3)
    }

    It 'measures an empty string as 0' {
        MDW '' | Should -Be 0
    }
}

Describe 'Test-AnsiSupport' -Tag 'Conversion' {
    BeforeAll {
        $saved = Save-ColorEnvironment
    }

    AfterAll {
        Restore-ColorEnvironment $saved
        Import-PWRSWriteColorEX
    }

    It 'answers <Expected> with <Name>' -TestCases @(
        @{ Name = 'FORCE_COLOR=3'; Values = @{ FORCE_COLOR = '3' }; Expected = 'TrueColor' }
        @{ Name = 'FORCE_COLOR=2'; Values = @{ FORCE_COLOR = '2' }; Expected = 'ANSI8' }
        @{ Name = 'FORCE_COLOR=1'; Values = @{ FORCE_COLOR = '1' }; Expected = 'ANSI4' }
        @{ Name = 'NO_COLOR'; Values = @{ NO_COLOR = '1' }; Expected = 'None' }
        @{ Name = 'TERM=dumb'; Values = @{ TERM = 'dumb' }; Expected = 'None' }
    ) {
        Set-ColorEnvironment $Values
        (Test-AnsiSupport -Silent).ColorSupport | Should -Be $Expected
    }

    It 'answers the shape PSWriteColorEX answers' {
        $result = Test-AnsiSupport -Silent
        @($result.PSObject.Properties.Name) | Should -Be @('ColorSupport', 'SupportsBoldFonts', 'Details')
        $result.Details.OperatingSystem | Should -BeOfType [System.PlatformID]
        $result.Details.PowerShellVersion | Should -Be $PSVersionTable.PSVersion.ToString()
        $result.Details.StyleSupport | Should -BeOfType [hashtable]
    }
}
