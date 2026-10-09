# Changelog

All notable changes to PWRSWriteColorEX are recorded here. The module is
published as `PWRSWriteColorEX` on the PowerShell Gallery, and a version heading
links to that version's page there. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Each release is one
commit, so this file, not the commit log, is the record of what changed in it.

PWRSWriteColorEX's version is PSWriteColorEX's: each release carries the
number of the PSWriteColorEX release whose commands and output it matches.

## [1.2.0] - 2026-10-09

The first release: the commands of
[PSWriteColorEX](https://github.com/MarkusMcNugen/PSWriteColorEX) 1.2.0
written in Rust with [PWRS (PoWerRuSt)](https://github.com/Variably-Constant/PWRS).

### Added

- `Write-ColorEX`, `Write-ColorError`, `Write-ColorWarning`,
  `Write-ColorInfo`, `Write-ColorSuccess`, `Write-ColorCritical`,
  `Write-ColorDebug`, `Format-ColorEX`, `Show-ColorTable`, `New-ColorStyle`,
  `Set-ColorDefault`, `Get-ColorProfiles`, `Export-ColorProfile`,
  `Import-ColorProfile`, `Remove-ColorProfile`, `Register-ColorName`,
  `Unregister-ColorName`, `Test-AnsiSupport`, `Measure-DisplayWidth`,
  `Convert-HexToRGB`, `Convert-RGBToANSI8`, `Convert-RGBToANSI4`,
  `Get-ColorTableWithRGB`, `Get-LighterRGBColor`, `Get-LighterColorName` and
  `Get-LighterANSI8Color`, with PSWriteColorEX 1.2.0's parameters, aliases
  and tab completion of color names and profile names.
- The `PSColorStyle` class, in C#, with PSWriteColorEX 1.2.0's properties,
  methods, static `Profiles` and `Default`, and built-in profiles.
  `[PSColorStyle]` resolves after `Import-Module`.
- `Write-ColorEX` writes what PSWriteColorEX 1.2.0 writes, escape code for
  escape code: markup tags, `-Split`, `-SplitAround` and `-SplitEvenly`,
  `-Highlight`, `-Link`, `-Reverse`, `-UnderlineStyle` and `-UnderlineColor`,
  gradients over the text and the background blended in OKLab to the last bit
  as PSWriteColorEX computes them, `-Truncate`, `-PadCenter` and `-Wrap`, and
  colors given as names, `#RGB`, `#RRGGBB`, `0xRGB`, `rgb()`, `hsl()`, RGB
  arrays and ANSI numbers.
- A profile file is the same bytes as PSWriteColorEX writes, so either module
  reads the other's.
- `Test-AnsiSupport` detects the terminals PSWriteColorEX 1.2.0 detects and
  reads the same color variables, comparing `TERM`, `TERM_PROGRAM`,
  `COLORTERM` and the host name without regard to case, as PowerShell's `-eq`
  and `-like` do.
- A Pester suite that compares the commands, parameters, aliases and
  `PSColorStyle` members with PSWriteColorEX 1.2.0's, and one that runs 495
  calls against both modules under eight color environments and compares what
  they write to the host, the warnings, errors and objects, and the log files.
- One module folder for Windows x64 and arm64, Linux x64 and arm64 (glibc
  2.35 or later), macOS arm64 and x64, and FreeBSD x64, for PowerShell 7.4 or
  later and Windows PowerShell 5.1.

[1.2.0]: https://www.powershellgallery.com/packages/PWRSWriteColorEX/1.2.0
