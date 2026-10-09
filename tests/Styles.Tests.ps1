# The Write-Color* helpers and the style profiles they read: PSColorStyle, New-ColorStyle,
# Set-ColorDefault and Get-ColorProfiles.

BeforeAll {
    . (Join-Path $PSScriptRoot 'Common.ps1')
    $saved = Save-ColorEnvironment
    # FORCE_COLOR writes a line of console colors alone as escape codes too, in every host
    Set-ColorEnvironment @{ FORCE_COLOR = '1' }
}

AfterAll {
    Restore-ColorEnvironment $saved
    Import-PWRSWriteColorEX
}

Describe 'The Write-Color* helpers' -Tag 'Styles' {
    AfterEach {
        [PSColorStyle]::Profiles = @{}
        [PSColorStyle]::InitializeDefaultProfiles()
    }

    It 'writes with the <Profile> profile, which has a style' -TestCases @(
        @{ Profile = 'Error'; Command = 'Write-ColorError'; Codes = "$([char]27)[1m$([char]27)[91m" }
        @{ Profile = 'Debug'; Command = 'Write-ColorDebug'; Codes = "$([char]27)[3m$([char]27)[90m" }
    ) {
        Get-HostText { & $Command 'message' } | Should -Be "$($Codes)message$esc[0m"
    }

    It 'writes with the <Profile> profile, a console color alone' -TestCases @(
        @{ Profile = 'Warning'; Command = 'Write-ColorWarning'; Code = 93 }
        @{ Profile = 'Info'; Command = 'Write-ColorInfo'; Code = 96 }
        @{ Profile = 'Success'; Command = 'Write-ColorSuccess'; Code = 92 }
    ) {
        Get-HostText { & $Command 'message' } | Should -Be "$esc[$($Code)mmessage$esc[0m"
    }

    It 'writes with the Critical profile, its dark red lighter where bold shows as brighter colors' {
        $background = if ((Test-AnsiSupport -Silent).SupportsBoldFonts) { 41 } else { 101 }
        Get-HostText { Write-ColorCritical 'message' } | Should -Be "$esc[1m$esc[5m$esc[97m$esc[$($background)mmessage$esc[0m"
    }

    It 'reads the profile at each call' {
        [PSColorStyle]::Profiles['Info'].ForegroundColor = 'Magenta'
        Get-HostText { Write-ColorInfo 'x' } | Should -Be "$esc[95mx$esc[0m"
    }

    It 'falls back to the colors a profile starts with when it was removed' {
        [PSColorStyle]::Profiles.Remove('Debug')
        Get-HostText { Write-ColorDebug 'x' } | Should -Be "$esc[3m$esc[90mx$esc[0m"
    }

    It 'writes the text to the pipeline with -PassThru' {
        'a', 'b' | Write-ColorSuccess -PassThru 6>$null | Should -Be @('a', 'b')
    }

    It 'writes each string piped to <Command> as a line of its own' -TestCases @(
        @{ Command = 'Write-ColorError' }
        @{ Command = 'Write-ColorWarning' }
        @{ Command = 'Write-ColorInfo' }
        @{ Command = 'Write-ColorSuccess' }
        @{ Command = 'Write-ColorCritical' }
        @{ Command = 'Write-ColorDebug' }
    ) {
        $writes = @(Get-HostWrite { 'a', 'b' | & $Command })
        $writes.Count | Should -Be 2
        Remove-EscapeCode $writes[0].Text | Should -Be 'a'
        Remove-EscapeCode $writes[1].Text | Should -Be 'b'
    }

    It 'answers to its aliases' {
        Get-HostText { WCE 'x' } | Should -Be "$esc[1m$esc[91mx$esc[0m"
    }
}

Describe 'PSColorStyle' -Tag 'Styles' {
    AfterEach {
        [PSColorStyle]::Profiles = @{}
        [PSColorStyle]::InitializeDefaultProfiles()
    }

    It 'starts with the seven built-in profiles' {
        $names = [string[]]@([PSColorStyle]::Profiles.Keys)
        [Array]::Sort($names, [StringComparer]::Ordinal)
        $names | Should -Be @('Critical', 'Debug', 'Default', 'Error', 'Info', 'Success', 'Warning')
    }

    It 'finds a profile without regard to case' {
        [PSColorStyle]::GetProfile('error').Name | Should -Be 'Error'
        [PSColorStyle]::Profiles['WARNING'].ForegroundColor | Should -Be 'Yellow'
    }

    It 'answers the Write-ColorEX parameters a style sets' {
        $style = [PSColorStyle]::new('Q', 'Red', 'Blue')
        $style.Bold = $true
        $style.StartTab = 2
        $params = $style.ToWriteColorParams()
        $keys = [string[]]@($params.Keys)
        [Array]::Sort($keys, [StringComparer]::Ordinal)
        $keys | Should -Be @('BackGroundColor', 'Bold', 'Color', 'StartTab')
        $params['color'] | Should -Be 'Red'
    }

    It 'sets Gradient in place of Color when it has two colors or more' {
        $style = [PSColorStyle]::new('G', 'Red', $null)
        $style.Gradient = @('Red', 'Blue')
        $params = $style.ToWriteColorParams()
        $params.ContainsKey('Color') | Should -BeFalse
        $params['Gradient'] | Should -Be @('Red', 'Blue')
    }

    It 'gives a copy its own arrays' {
        $style = [PSColorStyle]::new('C', @(1, 2, 3), $null)
        $style.Style = @('Bold')
        $copy = $style.Clone()
        $copy.Name | Should -Be 'C_Copy'
        $copy.ForegroundColor[0] = 9
        $copy.Style[0] = 'Italic'
        $style.ForegroundColor[0] | Should -Be 1
        $style.Style[0] | Should -Be 'Bold'
    }

    It 'reads a single style as an array of one' {
        $style = [PSColorStyle]::new()
        $style.Style = 'Bold'
        , $style.Style | Should -BeOfType [string[]]
        $style.Style.Count | Should -Be 1
    }
}

Describe 'New-ColorStyle, Set-ColorDefault and Get-ColorProfiles' -Tag 'Styles' {
    AfterEach {
        [PSColorStyle]::Profiles = @{}
        [PSColorStyle]::InitializeDefaultProfiles()
    }

    It 'makes a style from its parameters' {
        $style = New-ColorStyle -Name 'Full' -ForegroundColor Red -BackgroundColor Blue -Italic -StartSpaces 2 -AutoPad 9 -PadLeft -PadChar '*'
        $style | Should -BeOfType [PSColorStyle]
        $style.ForegroundColor | Should -Be 'Red'
        $style.BackgroundColor | Should -Be 'Blue'
        $style.Italic | Should -BeTrue
        $style.StartSpaces | Should -Be 2
        $style.AutoPad | Should -Be 9
        $style.PadLeft | Should -BeTrue
        $style.PadChar | Should -Be '*'
    }

    It 'makes Gray the text color when -ForegroundColor is left out' {
        (New-ColorStyle -Name 'Plain').ForegroundColor | Should -Be 'Gray'
    }

    It 'takes its first parameters by position' {
        $style = New-ColorStyle 'Pos' 'Red' 'Blue'
        $style.Name, $style.ForegroundColor, $style.BackgroundColor | Should -Be @('Pos', 'Red', 'Blue')
    }

    It 'adds a style to the profiles with -AddToProfiles' {
        $null = New-ColorStyle -Name 'Added' -AddToProfiles
        (Get-ColorProfiles -Name 'added').Name | Should -Be 'Added'
    }

    It 'makes a style the default with -SetAsDefault' {
        $style = New-ColorStyle -Name 'Mine' -SetAsDefault
        [PSColorStyle]::Default | Should -Be $style
    }

    It 'sets a new default style from its parameters' {
        Set-ColorDefault -ForegroundColor Cyan -Bold
        [PSColorStyle]::Default.ForegroundColor | Should -Be 'Cyan'
        [PSColorStyle]::Default.Bold | Should -BeTrue
        [PSColorStyle]::Profiles['Default'] | Should -Be ([PSColorStyle]::Default)
    }

    It 'sets a style as the default with -Style' {
        $style = New-ColorStyle -Name 'Other'
        Set-ColorDefault -Style $style
        [PSColorStyle]::Default | Should -Be $style
    }

    It 'makes the default plain Gray when called without parameters' {
        Set-ColorDefault -ForegroundColor Cyan -Bold
        Set-ColorDefault
        [PSColorStyle]::Default.ForegroundColor | Should -Be 'Gray'
        [PSColorStyle]::Default.BackgroundColor | Should -BeNullOrEmpty
        [PSColorStyle]::Default.Bold | Should -BeFalse
        [PSColorStyle]::Profiles['Default'] | Should -Be ([PSColorStyle]::Default)
    }

    It 'writes every profile' {
        @(Get-ColorProfiles).Count | Should -Be 7
    }

    It 'writes $null for a profile that does not exist' {
        $result = @(Get-ColorProfiles -Name 'missing')
        $result.Count | Should -Be 1
        $result[0] | Should -BeNullOrEmpty
    }
}
