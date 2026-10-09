<div align="center">

# PWRSWriteColorEX

**Colored and styled console output for PowerShell, written in Rust.**

[![PowerShell Gallery](https://img.shields.io/powershellgallery/v/PWRSWriteColorEX?label=gallery&style=flat-square)](https://www.powershellgallery.com/packages/PWRSWriteColorEX)
[![Documentation](https://img.shields.io/badge/docs-wiki-blue.svg?style=flat-square)](https://markusmcnugen.github.io/PSWriteColorEX/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](https://github.com/Variably-Constant/PWRSWriteColorEX/blob/main/LICENSE)
[![PowerShell](https://img.shields.io/badge/PowerShell-5.1%20%7C%207.4%2B-5391FE.svg?style=flat-square)](#platforms)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS%20%7C%20FreeBSD-555.svg?style=flat-square)](#platforms)

**[Documentation](https://markusmcnugen.github.io/PSWriteColorEX/)** | [Getting started](https://markusmcnugen.github.io/PSWriteColorEX/docs/tutorial/getting-started/) | [Write-ColorEX reference](https://markusmcnugen.github.io/PSWriteColorEX/docs/reference/write-colorex/) | [PWRSWriteColorEX](https://markusmcnugen.github.io/PSWriteColorEX/docs/explanation/pwrswritecolorex/)

PWRSWriteColorEX is [PSWriteColorEX](https://github.com/MarkusMcNugen/PSWriteColorEX) written in Rust with [PWRS (PoWerRuSt)](https://github.com/Variably-Constant/PWRS). It has the same commands, parameters, aliases and `PSColorStyle` class, and writes the same output, so a script written for PSWriteColorEX runs with it unchanged, in a fraction of the time. Its version is always PSWriteColorEX's.

</div>

---

<details>
<summary><b>Table of contents</b></summary>

- [Features](#features)
- [Quick start](#quick-start)
- [Commands](#commands)
- [Same output as PSWriteColorEX](#same-output-as-pswritecolorex)
- [Differences from PSWriteColorEX](#differences-from-pswritecolorex)
- [Performance](#performance)
- [How it is built](#how-it-is-built)
- [Building and testing](#building-and-testing)
- [Platforms](#platforms)
- [Wiki](#wiki)
- [Credits](#credits)
- [Use of AI tools](#use-of-ai-tools)
- [License](#license)

</details>

---

## Features

- TrueColor, 256 and 16 colors, and the console's colors where the terminal has no ANSI support, with the best mode the terminal has chosen for you. See [Terminal compatibility](https://markusmcnugen.github.io/PSWriteColorEX/docs/reference/terminals/).
- 129 color names in 44 families, hex codes, `rgb()`, `hsl()`, RGB arrays and color numbers, and names of your own with `Register-ColorName`.
- Gradients across the text and its background, blended in OKLab to the last bit as PSWriteColorEX blends them.
- Markup tags such as `[bold red]Error:[/]`, patterns colored where they match, strings split into colored parts, and links terminals open when clicked.
- Bold, faint, italic, blink, reverse, crossed-out and overlined text, and underlines single, double, curly, dotted or dashed in a color of their own.
- Padding, centering, cutting and wrapping that count wide characters, such as CJK and emoji, as 2 cells.
- Style profiles (`PSColorStyle`), the built-in profiles behind the six message helpers, a default style, and profile files either module reads.
- `Format-ColorEX` for colored text as strings, and `Show-ColorTable` for every name in every mode.
- Logging to a file under any of the 14 encoding names `-Encoding` takes, with the same bytes in Windows PowerShell 5.1 and PowerShell 7, and transcripts that record each line once.
- `NO_COLOR`, `FORCE_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE` and `TERM=dumb` honored.

## Quick start

Install the module from the PowerShell Gallery, or build it as described in [Building and testing](#building-and-testing):

```powershell
Install-Module PWRSWriteColorEX -Scope CurrentUser
```

Then write some color:

```powershell
Import-Module PWRSWriteColorEX

Write-ColorEX -Text 'Hello ', 'World' -Color Cyan, Green
Write-ColorEX -Text 'Warning: ', 'Disk space low' -Color Yellow, White -Bold
Write-ColorEX -Text 'RAINBOW' -Gradient Red, Orange, Yellow, Green, Cyan, Blue, Magenta
Write-ColorEX -Markup '[bold red]Error:[/] could not reach [cyan]db01[/]'
Write-ColorEX -Text 'Name', 'Value' -AutoPad 20 -Color Gray, White

Write-ColorError 'Operation failed'
Write-ColorSuccess 'Deployment completed' -LogFile 'deploy.log'
```

The [documentation](https://markusmcnugen.github.io/PSWriteColorEX/) covers both modules, with every example shown in the colors it writes; `Get-Help Write-ColorEX -Full` describes every parameter.

## Commands

| Command | Aliases |
|---|---|
| `Write-ColorEX` | `Write-Color`, `Write-Colour`, `Write-ColourEX`, `WC`, `WCEX`, `wcolor`, `wcolour` |
| `Write-ColorError` | `WCE`, `Write-ErrorColor`, `Write-ErrorColour`, `Write-ColourError`, `WError`, `wcerror` |
| `Write-ColorWarning` | `WCW`, `Write-WarningColor`, `Write-WarningColour`, `Write-ColourWarning`, `WWarning`, `WCWarn`, `wcwarning` |
| `Write-ColorInfo` | `WCI`, `Write-InfoColor`, `Write-InfoColour`, `Write-ColourInfo`, `WInfo`, `wcinfo` |
| `Write-ColorSuccess` | `WCS`, `Write-SuccessColor`, `Write-SuccessColour`, `Write-ColourSuccess`, `WSuccess`, `wcok`, `wcsuccess` |
| `Write-ColorCritical` | `WCC`, `Write-CriticalColor`, `Write-CriticalColour`, `Write-ColourCritical`, `WCritical`, `wccritical` |
| `Write-ColorDebug` | `WCD`, `Write-DebugColor`, `Write-DebugColour`, `Write-ColourDebug`, `WDebug`, `wcdebug` |
| `Format-ColorEX` | `FCEX`, `Format-ColourEX` |
| `Show-ColorTable` | `Show-ColourTable` |
| `New-ColorStyle` | `NCS`, `New-ColourStyle`, `New-Style`, `ncstyle` |
| `Set-ColorDefault` | `SCD`, `Set-ColourDefault`, `Set-DefaultColor`, `Set-DefaultColour` |
| `Get-ColorProfiles` | `GCP`, `Get-ColourProfiles`, `Get-Profiles`, `gcprofiles` |
| `Export-ColorProfile` | `Export-ColourProfile` |
| `Import-ColorProfile` | `Import-ColourProfile` |
| `Remove-ColorProfile` | `Remove-ColourProfile` |
| `Register-ColorName` | `Register-ColourName` |
| `Unregister-ColorName` | `Unregister-ColourName` |
| `Test-AnsiSupport` | `TAS`, `Test-ANSI` |
| `Measure-DisplayWidth` | `MDW`, `Get-DisplayWidth` |
| `Convert-HexToRGB` | `CHR`, `Hex2RGB` |
| `Convert-RGBToANSI8` | `CRA8`, `RGB2ANSI8` |
| `Convert-RGBToANSI4` | `CRA4`, `RGB2ANSI4` |
| `Get-ColorTableWithRGB` | `GCT`, `Get-ColorTable`, `Get-ColourTable` |
| `Get-LighterRGBColor` | `Lighten-RGBColor` |
| `Get-LighterColorName` | `Lighten-ColorName` |
| `Get-LighterANSI8Color` | `LA8`, `Lighten-ANSI8`, `Lighten-ANSI8Color` |

`[PSColorStyle]` resolves after `Import-Module`, with the static properties `Profiles` and `Default`, the static methods `GetProfile` and `InitializeDefaultProfiles`, and the instance methods `ToWriteColorParams`, `SetAsDefault`, `AddToProfiles` and `Clone`.

## Same output as PSWriteColorEX

Three suites hold the module to PSWriteColorEX 1.2.0, and CI runs them on every platform it tests:

- `tests/Surface.Tests.ps1` compares every command, parameter (type, position, aliases, valid values, parameter sets), alias and `PSColorStyle` member with what PSWriteColorEX 1.2.0 declares.
- `tests/Parity.Tests.ps1` runs the 495 calls in `tests/parity/cases.txt` against both modules, each in a process of its own, under eight color environments: TrueColor, 256 colors, 16 colors, `NO_COLOR`, `TERM=dumb`, `CLICOLOR=0`, `CLICOLOR_FORCE=1` with `TERM=dumb`, and the support the terminal reports. It compares each write to the host with its colors, each warning, error, verbose message and object, and the bytes of each log file.
- `tests/Transcript.Tests.ps1` checks that a transcript records each line once, as PSWriteColorEX does.

## Differences from PSWriteColorEX

- PowerShell 7 needs 7.4 or later, since the module's PowerShell 7 half is built for .NET 8. Windows PowerShell 5.1 works as with PSWriteColorEX.
- A color `-Color` or `-BackGroundColor` does not take is refused with a message of its own. PSWriteColorEX's message quotes its validation script.
- `-Debugging` writes the same messages. Without `-Verbose` they go to the host as verbose lines, which `4>` does not redirect. With `-Verbose` they are verbose records, as in PSWriteColorEX.
- When a log file cannot be written, the warning gives the reason in its own words, close to .NET's.
- The two modules cannot be imported into one session, since each defines `[PSColorStyle]`.

## Performance

Microseconds per call for both modules at 1.2.0, each call run 2,000 times after 200 warm-up calls with its output discarded, each module in a process of its own, in pwsh 7.6.6 on Windows 11 on an AMD Ryzen 9 7900X, with `FORCE_COLOR=3`:

| Call | PSWriteColorEX 1.2.0 | PWRSWriteColorEX 1.2.0 |
|---|---:|---:|
| `Write-ColorEX -Text 'Status: ', 'OK' -Color Gray, Green` | 748 | 175 |
| `Write-ColorEX -Text 'x' -Color '#FF8000' -Bold` | 616 | 186 |
| `Write-ColorEX -Text ('=' * 40) -Gradient Red, Blue` | 1,178 | 183 |
| `Write-ColorEX -Text '世界' -AutoPad 20` | 448 | 164 |
| `Write-ColorInfo 'message'` | 430 | 148 |

Most of the time left is spent in the .NET calls that build each write to the host. PWRS has no call for writing to the host yet, so the module builds the `HostInformationMessage` that `Write-Host` builds through PWRS's dynamic .NET access.

## How it is built

- `src/write.rs` is `Write-ColorEX`, step for step the same as PSWriteColorEX's, with `src/runs.rs` reading markup, splits and highlights into runs, and `src/helpers.rs` the `Write-Color*` helpers.
- `src/detect.rs` is `Test-AnsiSupport` and the color support detection, which on Windows turns on virtual terminal processing in the console as PSWriteColorEX does.
- `src/colors.rs` holds the color table and the conversions, with PowerShell's rounding of halves to even; `src/forms.rs` reads the color forms, `src/names.rs` the registered names, and `src/oklab.rs` blends gradients in OKLab.
- `src/width.rs` is `Measure-DisplayWidth`, from the table of the Rust crate [unicode-width](https://crates.io/crates/unicode-width) 0.2.2 and the emoji sequence rules, the same data PSWriteColorEX's is generated from. `tools/widthgen` writes `src/width_sets.rs` and PSWriteColorEX's table from that crate.
- `src/log.rs` writes the log file; `src/format.rs` is `Format-ColorEX`, `src/profiles.rs` the profile files, and `src/style.rs`, `src/convert.rs` and `src/gradient.rs` the style cmdlets, the conversion cmdlets and the gradient.
- `src/csharp/PSColorStyle.cs` is the `PSColorStyle` class, in C# beside the Rust because a PWRS class defined in Rust has no static properties, and `Profiles` and `Default` are static. `src/csharp/ModuleHooks.cs` registers `[PSColorStyle]` as a type accelerator at import, as PSWriteColorEX does, since PowerShell 7 loads the module's assembly in a load context of its own.

## Building and testing

The module builds with [PWRS](https://github.com/Variably-Constant/PWRS) 0.5.0 from crates.io, published as `PoWerRuSt`, and `cargo pwrs` of the same release:

```text
cargo install cargo-pwrs --version 0.5.0 --locked
cargo pwrs build --release      # the module folder in target/pwrs/PWRSWriteColorEX
cargo pwrs test --release       # the Rust tests, then the Pester suites
```

`cargo pwrs test` runs the Pester suites in PowerShell 7, and on Windows in Windows PowerShell 5.1 as well. The parity suite needs a checkout of [PSWriteColorEX](https://github.com/MarkusMcNugen/PSWriteColorEX), named by `PSWRITECOLOREX`:

```text
git clone https://github.com/MarkusMcNugen/PSWriteColorEX ../PSWriteColorEX
PSWRITECOLOREX=../PSWriteColorEX/PSWriteColorEX.psd1 cargo pwrs test --release
```

Rust 1.98 or later, edition 2024, and pwsh 7.4 or later on the building machine.

## Platforms

One module folder carries the native library of each platform under `runtimes/<rid>/native/`. `.github/workflows/ci.yml` builds each one and joins them with `cargo pwrs merge`:

| Platform | Built | Pester suites in CI |
|---|---|---|
| Windows x64 | on Windows | PowerShell 7 and Windows PowerShell 5.1 |
| Windows arm64 | on Windows x64, for arm64 | not run |
| Linux x64 | for glibc 2.35 with cargo-zigbuild | PowerShell 7 |
| Linux arm64 | for glibc 2.35 with cargo-zigbuild | not run |
| macOS arm64 | on macOS | PowerShell 7 |
| macOS x64 | on macOS arm64, for x64 | not run |
| FreeBSD x64 | in a FreeBSD 15.0 VM | PowerShell 7 |

The Linux libraries load on any distribution with glibc 2.35 or later. The x64 libraries are built for the baseline x86-64 CPU and the macOS arm64 library for the Apple A14, the oldest Apple Silicon core.

## Wiki

The [PSWriteColorEX documentation](https://markusmcnugen.github.io/PSWriteColorEX/) covers both modules: a tutorial, how-to guides, a reference for every command and parameter, and explanations, with every example run through the module and shown in the colors it writes. [PWRSWriteColorEX](https://markusmcnugen.github.io/PSWriteColorEX/docs/explanation/pwrswritecolorex/) there covers this module's platforms, differences and parity.

## Credits

- [PSWriteColorEX](https://github.com/MarkusMcNugen/PSWriteColorEX), the module this one matches, and whose design and behavior it follows.
- [PWRS](https://github.com/Variably-Constant/PWRS) binds the Rust cmdlets to PowerShell and builds the module.
- [unicode-width](https://crates.io/crates/unicode-width) gives the display width of each character.

## Use of AI tools

The author used Claude (Anthropic) via the Claude Code CLI for code development assistance and documentation drafting during the preparation of this repository. All design decisions and the final content were determined by the author. The implementation and the tests were verified through zero-warning `cargo clippy`, the Rust unit tests, and the Pester suites, the parity suite against PSWriteColorEX among them, in PowerShell 7 and Windows PowerShell 5.1.

## License

MIT, as written in [`LICENSE`](https://github.com/Variably-Constant/PWRSWriteColorEX/blob/main/LICENSE).
