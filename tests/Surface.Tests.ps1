# The commands, parameters, aliases and the PSColorStyle class match PSWriteColorEX 1.2.0's, so a
# script written for one module runs with the other. The expected text is what PSWriteColorEX
# 1.2.0 itself answers to the same functions.

BeforeAll {
    . (Join-Path $PSScriptRoot 'Common.ps1')
    Import-PWRSWriteColorEX

    # The commands of a module as text: each command with its aliases and parameter sets, and each
    # parameter with its type, position, whether it is mandatory or takes pipeline input, its
    # aliases, its valid values and range, and its parameter sets. Names sort ordinally, so every
    # host and culture writes the same text.
    function Get-CommandSurface {
        param([Parameter(Mandatory)][string]$Module)
        $ordinal = { param([string[]]$Items) if (-not $Items) { return @() }; $sorted = [string[]]$Items.Clone(); [Array]::Sort($sorted, [StringComparer]::Ordinal); $sorted }
        $common = [System.Management.Automation.Cmdlet]::CommonParameters + [System.Management.Automation.Cmdlet]::OptionalCommonParameters
        $commands = @(Get-Command -Module $Module -CommandType Function, Cmdlet)
        $names = & $ordinal @($commands | ForEach-Object Name)
        foreach ($name in $names) {
            $command = $commands | Where-Object Name -EQ $name
            $aliases = & $ordinal @(Get-Alias | Where-Object { $_.ResolvedCommandName -eq $name } | ForEach-Object Name)
            $sets = & $ordinal @($command.ParameterSets | ForEach-Object { $_.Name + $(if ($_.IsDefault) { '*' } else { '' }) })
            "$name aliases=[$($aliases -join ',')] sets=[$($sets -join ',')]"
            foreach ($parameter in $command.Parameters.Values) {
                if ($common -contains $parameter.Name) { continue }
                $attributes = @($parameter.Attributes | Where-Object { $_ -is [System.Management.Automation.ParameterAttribute] })
                $position = ($attributes | ForEach-Object { if ($_.Position -ge 0) { $_.Position } else { '-' } } | Select-Object -Unique) -join '/'
                $mandatory = ($attributes | ForEach-Object Mandatory | Select-Object -Unique) -join '/'
                $pipeline = ($attributes | ForEach-Object ValueFromPipeline | Select-Object -Unique) -join '/'
                $parameterSets = (& $ordinal @($attributes | ForEach-Object ParameterSetName | Select-Object -Unique)) -join '/'
                $validSet = ($parameter.Attributes | Where-Object { $_ -is [System.Management.Automation.ValidateSetAttribute] } | ForEach-Object { $_.ValidValues -join '|' })
                $range = ($parameter.Attributes | Where-Object { $_ -is [System.Management.Automation.ValidateRangeAttribute] } | ForEach-Object { "$($_.MinRange)..$($_.MaxRange)" })
                $parameterAliases = (& $ordinal @($parameter.Aliases)) -join ','
                "  $($parameter.Name) type=$($parameter.ParameterType.Name) position=$position mandatory=$mandatory pipeline=$pipeline aliases=[$parameterAliases] values=[$validSet] range=[$range] sets=[$parameterSets]"
            }
        }
    }

    # The members of [PSColorStyle] as text: its instance members, its static members and its
    # constructors.
    function Get-ClassSurface {
        $ordinal = { param($Members) $Members | Sort-Object { $_.Name } -CaseSensitive | ForEach-Object { "$($_.Name) $($_.MemberType) $($_.Definition)" } }
        '# instance'
        & $ordinal @([PSColorStyle]::new() | Get-Member)
        '# static'
        & $ordinal @([PSColorStyle] | Get-Member -Static)
    }
}

Describe 'The surface of PSWriteColorEX 1.2.0' -Tag 'Surface' {
    It 'has the same commands, parameters and aliases' {
        $expected = @'
Convert-HexToRGB aliases=[CHR,Hex2RGB] sets=[__AllParameterSets]
  Hex type=String position=0 mandatory=True pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Convert-RGBToANSI4 aliases=[CRA4,RGB2ANSI4] sets=[__AllParameterSets]
  RGB type=Int32[] position=0 mandatory=True pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Convert-RGBToANSI8 aliases=[CRA8,RGB2ANSI8] sets=[__AllParameterSets]
  RGB type=Int32[] position=0 mandatory=True pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Export-ColorProfile aliases=[Export-ColourProfile] sets=[__AllParameterSets]
  Path type=String position=0 mandatory=True pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Name type=String[] position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Format-ColorEX aliases=[FCEX,Format-ColourEX] sets=[__AllParameterSets]
  Text type=String[] position=0 mandatory=False pipeline=True aliases=[T] values=[] range=[] sets=[__AllParameterSets]
  Color type=Array position=1 mandatory=False pipeline=False aliases=[C,FGC,ForegroundColor] values=[] range=[] sets=[__AllParameterSets]
  BackGroundColor type=Array position=2 mandatory=False pipeline=False aliases=[B,BGC] values=[] range=[] sets=[__AllParameterSets]
  Gradient type=Object[] position=- mandatory=False pipeline=False aliases=[Grad] values=[] range=[] sets=[__AllParameterSets]
  BackGroundGradient type=Object[] position=- mandatory=False pipeline=False aliases=[BGGrad] values=[] range=[] sets=[__AllParameterSets]
  GradientSpace type=String position=- mandatory=False pipeline=False aliases=[] values=[OKLab|RGB] range=[] sets=[__AllParameterSets]
  ANSI4 type=SwitchParameter position=- mandatory=False pipeline=False aliases=[A4] values=[] range=[] sets=[__AllParameterSets]
  ANSI8 type=SwitchParameter position=- mandatory=False pipeline=False aliases=[A8] values=[] range=[] sets=[__AllParameterSets]
  ANSI24 type=SwitchParameter position=- mandatory=False pipeline=False aliases=[A24,TC,TrueColor] values=[] range=[] sets=[__AllParameterSets]
  Style type=Object position=- mandatory=False pipeline=False aliases=[S] values=[] range=[] sets=[__AllParameterSets]
  StyleProfile type=PSColorStyle position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Default type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Bold type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Faint type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Italic type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Underline type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Blink type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  CrossedOut type=SwitchParameter position=- mandatory=False pipeline=False aliases=[Strikethrough] values=[] range=[] sets=[__AllParameterSets]
  DoubleUnderline type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Overline type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Reverse type=SwitchParameter position=- mandatory=False pipeline=False aliases=[Invert] values=[] range=[] sets=[__AllParameterSets]
  UnderlineColor type=Array position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  UnderlineStyle type=String position=- mandatory=False pipeline=False aliases=[] values=[Single|Double|Curly|Dotted|Dashed] range=[] sets=[__AllParameterSets]
  Markup type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Split type=String[] position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  SplitAround type=String[] position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  SplitEvenly type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Highlight type=IDictionary position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Link type=String[] position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  StartTab type=Int32 position=- mandatory=False pipeline=False aliases=[Indent] values=[] range=[] sets=[__AllParameterSets]
  StartSpaces type=Int32 position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  AutoPad type=Int32 position=- mandatory=False pipeline=False aliases=[Pad,PadWidth] values=[] range=[] sets=[__AllParameterSets]
  PadLeft type=SwitchParameter position=- mandatory=False pipeline=False aliases=[RightAlign] values=[] range=[] sets=[__AllParameterSets]
  PadCenter type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  PadChar type=Char position=- mandatory=False pipeline=False aliases=[FillChar,PaddingChar] values=[] range=[] sets=[__AllParameterSets]
  Truncate type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Wrap type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Debugging type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Silent type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Get-ColorProfiles aliases=[GCP,Get-ColourProfiles,Get-Profiles,gcprofiles] sets=[__AllParameterSets]
  Name type=String position=0 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Get-ColorTableWithRGB aliases=[GCT,Get-ColorTable,Get-ColourTable] sets=[__AllParameterSets]
Get-LighterANSI8Color aliases=[LA8,Lighten-ANSI8,Lighten-ANSI8Color] sets=[__AllParameterSets]
  ANSI8Code type=Int32 position=0 mandatory=True pipeline=False aliases=[] values=[] range=[0..255] sets=[__AllParameterSets]
  Factor type=Double position=1 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Get-LighterColorName aliases=[Lighten-ColorName] sets=[__AllParameterSets]
  ColorName type=String position=0 mandatory=True pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Get-LighterRGBColor aliases=[Lighten-RGBColor] sets=[__AllParameterSets]
  RGB type=Int32[] position=0 mandatory=True pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Factor type=Double position=1 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Import-ColorProfile aliases=[Import-ColourProfile] sets=[__AllParameterSets]
  Path type=String position=0 mandatory=True pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  PassThru type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Measure-DisplayWidth aliases=[Get-DisplayWidth,MDW] sets=[__AllParameterSets]
  Text type=String position=0 mandatory=True pipeline=True aliases=[] values=[] range=[] sets=[__AllParameterSets]
  AmbiguousAsWide type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
New-ColorStyle aliases=[NCS,New-ColourStyle,New-Style,ncstyle] sets=[__AllParameterSets]
  Name type=String position=0 mandatory=True pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  ForegroundColor type=Object position=1 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  BackgroundColor type=Object position=2 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Gradient type=Object[] position=3 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Bold type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Italic type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Underline type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Blink type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Faint type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  CrossedOut type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  DoubleUnderline type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Overline type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  ShowTime type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  NoNewLine type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  HorizontalCenter type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  StartTab type=Int32 position=4 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  StartSpaces type=Int32 position=5 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  LinesBefore type=Int32 position=6 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  LinesAfter type=Int32 position=7 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  AutoPad type=Int32 position=8 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  PadLeft type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  PadChar type=Char position=9 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  BackgroundGradient type=Object[] position=10 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  GradientSpace type=String position=11 mandatory=False pipeline=False aliases=[] values=[OKLab|RGB] range=[] sets=[__AllParameterSets]
  Reverse type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  UnderlineColor type=Object position=12 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  UnderlineStyle type=String position=13 mandatory=False pipeline=False aliases=[] values=[Single|Double|Curly|Dotted|Dashed] range=[] sets=[__AllParameterSets]
  PadCenter type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Truncate type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Wrap type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  AddToProfiles type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  SetAsDefault type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Register-ColorName aliases=[Register-ColourName] sets=[__AllParameterSets]
  Name type=String position=0 mandatory=True pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Color type=Object position=1 mandatory=True pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Force type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Remove-ColorProfile aliases=[Remove-ColourProfile] sets=[__AllParameterSets]
  Name type=String[] position=0 mandatory=True pipeline=True aliases=[] values=[] range=[] sets=[__AllParameterSets]
Set-ColorDefault aliases=[SCD,Set-ColourDefault,Set-DefaultColor,Set-DefaultColour] sets=[Object,Properties*]
  Style type=PSColorStyle position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[Object]
  ForegroundColor type=Object position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[Properties]
  BackgroundColor type=Object position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[Properties]
  Bold type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[Properties]
  Italic type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[Properties]
  Underline type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[Properties]
  ShowTime type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[Properties]
  StartTab type=Int32 position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[Properties]
  StartSpaces type=Int32 position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[Properties]
Show-ColorTable aliases=[Show-ColourTable] sets=[__AllParameterSets]
  Name type=String[] position=0 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Background type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Test-AnsiSupport aliases=[TAS,Test-ANSI] sets=[__AllParameterSets]
  Silent type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Unregister-ColorName aliases=[Unregister-ColourName] sets=[__AllParameterSets]
  Name type=String[] position=0 mandatory=True pipeline=True aliases=[] values=[] range=[] sets=[__AllParameterSets]
Write-ColorCritical aliases=[WCC,WCritical,Write-ColourCritical,Write-CriticalColor,Write-CriticalColour,wccritical] sets=[__AllParameterSets]
  Text type=String[] position=0 mandatory=False pipeline=True aliases=[] values=[] range=[] sets=[__AllParameterSets]
  NoNewLine type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  LogFile type=String position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  NoConsoleOutput type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  PassThru type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Write-ColorDebug aliases=[WCD,WDebug,Write-ColourDebug,Write-DebugColor,Write-DebugColour,wcdebug] sets=[__AllParameterSets]
  Text type=String[] position=0 mandatory=False pipeline=True aliases=[] values=[] range=[] sets=[__AllParameterSets]
  NoNewLine type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  LogFile type=String position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  NoConsoleOutput type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  PassThru type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Write-ColorEX aliases=[WC,WCEX,Write-Color,Write-Colour,Write-ColourEX,wcolor,wcolour] sets=[__AllParameterSets]
  Text type=String[] position=0 mandatory=False pipeline=True aliases=[T] values=[] range=[] sets=[__AllParameterSets]
  Color type=Array position=1 mandatory=False pipeline=False aliases=[C,FGC,ForegroundColor] values=[] range=[] sets=[__AllParameterSets]
  BackGroundColor type=Array position=2 mandatory=False pipeline=False aliases=[B,BGC] values=[] range=[] sets=[__AllParameterSets]
  Gradient type=Object[] position=3 mandatory=False pipeline=False aliases=[Grad] values=[] range=[] sets=[__AllParameterSets]
  ANSI4 type=SwitchParameter position=- mandatory=False pipeline=False aliases=[A4] values=[] range=[] sets=[__AllParameterSets]
  ANSI8 type=SwitchParameter position=- mandatory=False pipeline=False aliases=[A8] values=[] range=[] sets=[__AllParameterSets]
  ANSI24 type=SwitchParameter position=- mandatory=False pipeline=False aliases=[A24,TC,TrueColor] values=[] range=[] sets=[__AllParameterSets]
  Style type=Object position=4 mandatory=False pipeline=False aliases=[S] values=[] range=[] sets=[__AllParameterSets]
  StyleProfile type=PSColorStyle position=5 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Default type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Bold type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Faint type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Italic type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Underline type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Blink type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  CrossedOut type=SwitchParameter position=- mandatory=False pipeline=False aliases=[Strikethrough] values=[] range=[] sets=[__AllParameterSets]
  DoubleUnderline type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Overline type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  StartTab type=Int32 position=6 mandatory=False pipeline=False aliases=[Indent] values=[] range=[] sets=[__AllParameterSets]
  LinesBefore type=Int32 position=7 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  LinesAfter type=Int32 position=8 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  StartSpaces type=Int32 position=9 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  LogFile type=String position=10 mandatory=False pipeline=False aliases=[L] values=[] range=[] sets=[__AllParameterSets]
  LogPath type=String position=11 mandatory=False pipeline=False aliases=[LP] values=[] range=[] sets=[__AllParameterSets]
  LogLevel type=String position=12 mandatory=False pipeline=False aliases=[LL,LogLvl] values=[] range=[] sets=[__AllParameterSets]
  LogTime type=SwitchParameter position=- mandatory=False pipeline=False aliases=[LT] values=[] range=[] sets=[__AllParameterSets]
  DateTimeFormat type=String position=13 mandatory=False pipeline=False aliases=[DateFormat,TS,TimeFormat,Timestamp] values=[] range=[] sets=[__AllParameterSets]
  LogRetry type=Int32 position=14 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Encoding type=String position=15 mandatory=False pipeline=False aliases=[] values=[unknown|string|unicode|bigendianunicode|utf8|utf8BOM|utf8NoBOM|utf7|utf32|bigendianutf32|ascii|ansi|default|oem] range=[] sets=[__AllParameterSets]
  ShowTime type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  NoNewLine type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  HorizontalCenter type=SwitchParameter position=- mandatory=False pipeline=False aliases=[Center] values=[] range=[] sets=[__AllParameterSets]
  BlankLine type=SwitchParameter position=- mandatory=False pipeline=False aliases=[BL,Blank,Empty] values=[] range=[] sets=[__AllParameterSets]
  NoConsoleOutput type=SwitchParameter position=- mandatory=False pipeline=False aliases=[HideConsole,LO,LogOnly,NoConsole] values=[] range=[] sets=[__AllParameterSets]
  Debugging type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Silent type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  AutoPad type=Int32 position=16 mandatory=False pipeline=False aliases=[Pad,PadWidth] values=[] range=[] sets=[__AllParameterSets]
  PadLeft type=SwitchParameter position=- mandatory=False pipeline=False aliases=[RightAlign] values=[] range=[] sets=[__AllParameterSets]
  PadChar type=Char position=17 mandatory=False pipeline=False aliases=[FillChar,PaddingChar] values=[] range=[] sets=[__AllParameterSets]
  Markup type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Split type=String[] position=18 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  SplitAround type=String[] position=19 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  SplitEvenly type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Highlight type=IDictionary position=20 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Link type=String[] position=21 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Reverse type=SwitchParameter position=- mandatory=False pipeline=False aliases=[Invert] values=[] range=[] sets=[__AllParameterSets]
  BackGroundGradient type=Object[] position=22 mandatory=False pipeline=False aliases=[BGGrad] values=[] range=[] sets=[__AllParameterSets]
  GradientSpace type=String position=23 mandatory=False pipeline=False aliases=[] values=[OKLab|RGB] range=[] sets=[__AllParameterSets]
  UnderlineColor type=Array position=24 mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  UnderlineStyle type=String position=25 mandatory=False pipeline=False aliases=[] values=[Single|Double|Curly|Dotted|Dashed] range=[] sets=[__AllParameterSets]
  Truncate type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  PadCenter type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  Wrap type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Write-ColorError aliases=[WCE,WError,Write-ColourError,Write-ErrorColor,Write-ErrorColour,wcerror] sets=[__AllParameterSets]
  Text type=String[] position=0 mandatory=False pipeline=True aliases=[] values=[] range=[] sets=[__AllParameterSets]
  NoNewLine type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  LogFile type=String position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  NoConsoleOutput type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  PassThru type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Write-ColorInfo aliases=[WCI,WInfo,Write-ColourInfo,Write-InfoColor,Write-InfoColour,wcinfo] sets=[__AllParameterSets]
  Text type=String[] position=0 mandatory=False pipeline=True aliases=[] values=[] range=[] sets=[__AllParameterSets]
  NoNewLine type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  LogFile type=String position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  NoConsoleOutput type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  PassThru type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Write-ColorSuccess aliases=[WCS,WSuccess,Write-ColourSuccess,Write-SuccessColor,Write-SuccessColour,wcok,wcsuccess] sets=[__AllParameterSets]
  Text type=String[] position=0 mandatory=False pipeline=True aliases=[] values=[] range=[] sets=[__AllParameterSets]
  NoNewLine type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  LogFile type=String position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  NoConsoleOutput type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  PassThru type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
Write-ColorWarning aliases=[WCW,WCWarn,WWarning,Write-ColourWarning,Write-WarningColor,Write-WarningColour,wcwarning] sets=[__AllParameterSets]
  Text type=String[] position=0 mandatory=False pipeline=True aliases=[] values=[] range=[] sets=[__AllParameterSets]
  NoNewLine type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  LogFile type=String position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  NoConsoleOutput type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
  PassThru type=SwitchParameter position=- mandatory=False pipeline=False aliases=[] values=[] range=[] sets=[__AllParameterSets]
'@
        (Get-CommandSurface -Module 'PWRSWriteColorEX') -join "`n" | Should -BeExactly ($expected -replace "`r", '')
    }

    It 'has the same PSColorStyle members' {
        $expected = @'
# instance
AddToProfiles Method void AddToProfiles()
AutoPad Property int AutoPad {get;set;}
BackgroundColor Property System.Object BackgroundColor {get;set;}
BackgroundGradient Property System.Object[] BackgroundGradient {get;set;}
Blink Property bool Blink {get;set;}
Bold Property bool Bold {get;set;}
Clone Method PSColorStyle Clone()
CrossedOut Property bool CrossedOut {get;set;}
DoubleUnderline Property bool DoubleUnderline {get;set;}
Equals Method bool Equals(System.Object obj)
Faint Property bool Faint {get;set;}
ForegroundColor Property System.Object ForegroundColor {get;set;}
GetHashCode Method int GetHashCode()
GetType Method type GetType()
Gradient Property System.Object[] Gradient {get;set;}
GradientSpace Property string GradientSpace {get;set;}
HorizontalCenter Property bool HorizontalCenter {get;set;}
Italic Property bool Italic {get;set;}
LinesAfter Property int LinesAfter {get;set;}
LinesBefore Property int LinesBefore {get;set;}
Name Property string Name {get;set;}
NoNewLine Property bool NoNewLine {get;set;}
Overline Property bool Overline {get;set;}
PadCenter Property bool PadCenter {get;set;}
PadChar Property char PadChar {get;set;}
PadLeft Property bool PadLeft {get;set;}
Reverse Property bool Reverse {get;set;}
SetAsDefault Method void SetAsDefault()
ShowTime Property bool ShowTime {get;set;}
StartSpaces Property int StartSpaces {get;set;}
StartTab Property int StartTab {get;set;}
Style Property string[] Style {get;set;}
ToString Method string ToString()
ToWriteColorParams Method hashtable ToWriteColorParams()
Truncate Property bool Truncate {get;set;}
Underline Property bool Underline {get;set;}
UnderlineColor Property System.Object UnderlineColor {get;set;}
UnderlineStyle Property string UnderlineStyle {get;set;}
Wrap Property bool Wrap {get;set;}
# static
Default Property static PSColorStyle Default {get;set;}
Equals Method static bool Equals(System.Object objA, System.Object objB)
GetProfile Method static PSColorStyle GetProfile(string name)
InitializeDefaultProfiles Method static void InitializeDefaultProfiles()
new Method PSColorStyle new(), PSColorStyle new(string name), PSColorStyle new(string name, System.Object foreground, System.Object background)
Profiles Property static hashtable Profiles {get;set;}
ReferenceEquals Method static bool ReferenceEquals(System.Object objA, System.Object objB)
'@
        (Get-ClassSurface) -join "`n" | Should -BeExactly ($expected -replace "`r", '')
    }

    It 'resolves [PSColorStyle] after Import-Module, as PSWriteColorEX does' {
        [PSColorStyle]::Profiles.Count | Should -Be 7
        [PSColorStyle]::Default.Name | Should -Be 'Default'
    }
}
