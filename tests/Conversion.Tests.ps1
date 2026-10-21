# The color conversions, the color table, Measure-DisplayWidth and Test-AnsiSupport.

BeforeDiscovery {
    # Each case: a name, the code points of the text, its width, and its width with -AmbiguousAsWide
    $script:widthCases = @(
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

    It 'returns gray with a warning for an invalid code' {
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

    It 'works under the Lighten-* names' {
        Lighten-ColorName 'DarkBlue' | Should -Be 'Blue'
        LA8 1 | Should -Be 9
    }
}

Describe 'Get-ColorContrast' -Tag 'Conversion' {
    It 'returns the WCAG contrast ratio, from 1 to 21' {
        $ratio = Get-ColorContrast White Black
        $ratio | Should -BeOfType [double]
        [Math]::Abs($ratio - 21) | Should -BeLessThan 1e-12
        Get-ColorContrast Crimson Crimson | Should -Be 1
        [Math]::Abs((Get-ColorContrast Red White) - 3.998) | Should -BeLessThan 0.001
        [Math]::Abs((Get-ColorContrast @(229, 229, 229) Black) - 16.67) | Should -BeLessThan 0.01
    }

    It 'gives the same ratio either way round' {
        $one = Get-ColorContrast Gold '#1E1E2E'
        $two = Get-ColorContrast '#1E1E2E' Gold
        [BitConverter]::DoubleToInt64Bits($one) | Should -Be ([BitConverter]::DoubleToInt64Bits($two))
    }

    It 'takes a color in each form Register-ColorName takes' {
        $expected = Get-ColorContrast @(255, 128, 0) @(0, 0, 128)
        Get-ColorContrast '#FF8000' Navy | Should -Be $expected
        Get-ColorContrast 'rgb(255, 128, 0)' '#000080' | Should -Be $expected
        Get-ColorContrast 'hsl(30, 100%, 50%)' 'rgb(0, 0, 128)' | Should -Be $expected
        Get-ColorContrast '#F80' Navy | Should -Be (Get-ColorContrast @(255, 136, 0) Navy)
        Register-ColorName Brand '#FF8000'
        try {
            Get-ColourContrast -C Brand -BGC Navy | Should -Be $expected
        } finally {
            Unregister-ColorName Brand
        }
    }

    It 'gives an error and no ratio for a value that is not a color' {
        $ratio = Get-ColorContrast NoSuchColor White -ErrorVariable failures -ErrorAction SilentlyContinue
        $ratio | Should -BeNullOrEmpty
        $failures.Count | Should -Be 1
        $failures[0].FullyQualifiedErrorId | Should -BeLike 'InvalidColor*'
        $failures[0].Exception.Message | Should -Be "'NoSuchColor' is not a color. A color is a hex code, rgb(), hsl(), an RGB array of three numbers 0-255, or a name in the color table."
        $null = Get-ColorContrast @(1, 2) @(256, 0, 0) -ErrorVariable both -ErrorAction SilentlyContinue
        @($both | ForEach-Object { $_.Exception.Message.Split(' ')[0] }) | Should -Be @("'@(1,", "'@(256,")
    }
}

Describe 'Get-ColorTableWithRGB' -Tag 'Conversion' {
    BeforeAll {
        $script:table = Get-ColorTableWithRGB
    }

    It 'holds 129 color names' {
        $script:table | Should -BeOfType [hashtable]
        $script:table.Count | Should -Be 129
    }

    It 'finds a name without regard to case' {
        $script:table['ORANGE'][3] | Should -Be 214
    }

    It 'holds each color in every mode' {
        $entry = $script:table['DarkRed']
        $entry[0] | Should -Be 'DarkRed'
        $entry[1] | Should -Be 31
        $entry[2] | Should -Be 41
        $entry[3] | Should -Be 88
        $entry[4] | Should -Be @(139, 0, 0)
        $entry[1] | Should -BeOfType [int]
    }
}

Describe 'Measure-DisplayWidth' -Tag 'Conversion' {
    It 'measures <Name> as <Width> cells' -TestCases $script:widthCases {
        Measure-DisplayWidth -Text (ConvertTo-TestString $CodePoints) | Should -Be $Width
    }

    It 'measures <Name> as <WideWidth> cells with -AmbiguousAsWide' -TestCases $script:widthCases {
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
        $script:saved = Save-ColorEnvironment
    }

    AfterAll {
        Restore-ColorEnvironment $script:saved
        Import-PWRSWriteColorEX
    }

    It 'returns <Expected> for <Name>' -TestCases @(
        @{ Name = 'FORCE_COLOR=3'; Values = @{ FORCE_COLOR = '3' }; Expected = 'TrueColor' }
        @{ Name = 'FORCE_COLOR=2'; Values = @{ FORCE_COLOR = '2' }; Expected = 'ANSI8' }
        @{ Name = 'FORCE_COLOR=1'; Values = @{ FORCE_COLOR = '1' }; Expected = 'ANSI4' }
        @{ Name = 'NO_COLOR'; Values = @{ NO_COLOR = '1' }; Expected = 'None' }
        @{ Name = 'TERM=dumb'; Values = @{ TERM = 'dumb' }; Expected = 'None' }
    ) {
        Set-ColorEnvironment $Values
        (Test-AnsiSupport -Silent).ColorSupport | Should -Be $Expected
    }

    It 'returns the shape PSWriteColorEX returns' {
        $result = Test-AnsiSupport -Silent
        @($result.PSObject.Properties.Name) | Should -Be @('ColorSupport', 'SupportsBoldFonts', 'SupportsUnicode', 'Details')
        $result.SupportsUnicode | Should -BeOfType [bool]
        $result.Details.OperatingSystem | Should -BeOfType [System.PlatformID]
        $result.Details.PowerShellVersion | Should -Be $PSVersionTable.PSVersion.ToString()
        $result.Details.StyleSupport | Should -BeOfType [hashtable]
    }
}
