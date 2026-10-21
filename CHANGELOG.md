# Changelog

All notable changes to PWRSWriteColorEX are recorded here. The module is
published as `PWRSWriteColorEX` on the PowerShell Gallery, and a version heading
links to that version's page there. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Each release is one
commit, so this file, not the commit log, is the record of what changed in it.

PWRSWriteColorEX's major and minor version are those of the PSWriteColorEX
release whose commands and output it matches; its patch number may differ from
PSWriteColorEX's.

## [1.2.1] - 2026-10-21

The commands, parameters and output of PSWriteColorEX 1.2.1.

### Added

- `-Gradient` and `-BackGroundGradient` take the name of a named gradient in
  place of its colors, and so do a style's `Gradient` and `BackgroundGradient`:
  twenty-one names, among them Rainbow, Sunset, Ocean, the presets of
  gradient-string, and Progress, the color table's Red, LightRuby, Orange,
  Yellow and Green. Tab completion lists them.
- Gradient options, alike in `Write-ColorEX`, `Format-ColorEX`, `New-ColorStyle`
  and a style's properties: `-GradientSpace` takes LinearRGB, OKLCH, HSL, HSV,
  Lab and LCh besides OKLab and RGB; `-GradientHue` picks the way round the hue
  wheel (Shorter, Longer, Increasing, Decreasing); `-GradientEasing` moves the
  colors along the text by one of 31 curves; `-GradientRepeat` repeats or
  mirrors stops placed short of the ends; `-GradientDirection` runs a gradient
  in reverse, from the middle out or from the ends in; and `-GradientSteps` cuts
  it into bands of solid color. Without them a gradient is drawn as before.
- A gradient color can give its place after an `@`, as a percent: `'Gold@40'`.
  Colors without a place spread evenly between the placed ones, as CSS places
  color stops, and each lands on the nearest character.
- `Get-GradientColor`, with the alias `Get-GradientColour`, returns a gradient's
  colors as RGB arrays, with every gradient option: one for each of `-Count`
  characters, or the color at each of `-Place`'s places, from 0 to 1, in the
  order given.
- The library's `gradient::gradient_rgb` and `gradient::gradient_rgb_at`
  compute a gradient's colors without a pipeline, so another module may call
  them from a thread that never touches PowerShell; the `spaces` and `ease`
  modules hold the color spaces and the easing curves.
- `Get-ColorContrast`, with the alias `Get-ColourContrast`, returns the WCAG
  contrast ratio of a `-Color` on a `-BackGroundColor`, from 1 to 21, the same
  to the last bit as PSWriteColorEX's; the library's `oklab::contrast_ratio`
  computes it for another module.
- `Test-AnsiSupport` reports `SupportsUnicode`, whether the terminal can show
  Unicode: `WRITECOLOREX_UNICODE`, `true` or `false`, decides first;
  otherwise it is `$false` on Linux, macOS and FreeBSD when the locale's
  character set is not UTF-8, and on Windows when the console is a window of
  the console host itself drawing with a raster font. The console's own screen
  buffer is asked, so redirected output does not change the answer. The
  library's `detect::unicode_support` gives the same answer without a
  pipeline, and `detect::Detection` carries it as `unicode`.
- `-ColorSupport` on `Write-ColorEX` and `Format-ColorEX` names the color
  support to write for, None, ANSI4, ANSI8 or TrueColor, in place of the
  detection, the host, redirected output and the color variables, so the
  colors depend only on the arguments. `-Bold` lightens the colors wherever
  bold cannot show: always with None, and with the others where the terminal
  draws bold as brighter colors.
- The library's `write::WriteOptions` has `color_support`, an
  `Option<detect::Support>`, for a module that calls `write::render`: set, it
  takes the place of the detection and the color variables, as `-ColorSupport`
  does.
- `Import-ColorProfile -Theme` reads one of the module's themes. Catppuccin
  (Latte, Frappe, Macchiato, Mocha), Solarized (dark, light), Gruvbox (dark,
  light), Dracula, Nord, One Dark and Tokyo Night restyle the seven profiles
  and register their colors under names such as `DraculaPurple`. `PowerShell`
  makes Error, Warning, Info and Debug, and on PowerShell 7.2 and later
  Critical and Default, exactly PowerShell's own settings. `CSS` adds the 100
  CSS color names the table lacks. The themes are built into the library.
- `Show-ColorTable -Styles`, `-Palette` and `-Profiles` write the text styles
  with the parameters and markup tags that give them, the 256-color chart, and
  the style profiles with their settings.
- The `about_PWRSWriteColorEX` help topic: a cheat sheet of the commands and
  parameters by task, the same as PSWriteColorEX's and the wiki's.
- The module's THIRD-PARTY-NOTICES.txt credits the sources of the themes' and
  the named gradients' colors.
- The crate also builds as a Rust library another PWRS module can link. The
  color, color form, gradient, OKLab, conversion, detection, name, style, width,
  run and write modules are public, and the module's exports sit behind the
  `module` feature, on by default, which such a module turns off with
  `default-features = false`.

### Changed

- The module builds with PWRS 0.7.0 from crates.io, published as `PoWerRuSt`,
  and `cargo pwrs` of the same release.
- Each write to the host goes through PWRS's `Pipeline::write_host`, one call
  into .NET. The module built `Write-Host`'s `HostInformationMessage` itself
  through PowerShell's member binder before, and in Windows PowerShell 5.1
  reached the host's internal `TranscribeResult` by reflection for the
  transcript. What reaches the host, the information stream, redirections and
  transcripts is the same.
- `Write-ColorEX` and the `Write-Color*` helpers take 9% to 20% less time per
  call than in 1.2.0: 135 microseconds rather than 165 for two colored
  segments, and 131 rather than 148 for a hex color with `-Bold`, each call run
  2,000 times on Windows 11 in pwsh 7.6.7.
- In 256 colors, a color is written as the code that looks nearest to it: the
  code at the least distance in OKLab, a color space where equal distances look
  equally different, with the difference in lightness counted at 0.6 of its
  size. Hex codes, `rgb()`, `hsl()`, RGB arrays, registered names and gradients
  all go through it, in `Convert-RGBToANSI8`. Each channel was rounded on its
  own before, which made saturated colors pale and could turn a violet blue.
- A color name gets the same 256-color code as its hex code. The 17 names on
  codes 0 to 15 keep them, so they follow the terminal's scheme. 74 names
  changed code, among them Lime (118 to 46, its exact color), Purple (93 to
  90), Yellow (220 to 226) and DarkSky (24 to 39).
- Each stop of a gradient falls on a character of its own when the text has at
  least as many characters as stops, so every color given is drawn exactly.
  Gradients of three or more stops change slightly in TrueColor too.
- A text gradient leaves out white space, which shows no text color, unless it
  is underlined, crossed out, overlined or reversed. A background gradient
  still colors every character.
- The `Write-Color*` helpers declare that they return strings, which
  `-PassThru` sends down the pipeline, as every other command that returns
  objects already did. They declared nothing before.
- No thread waits on another inside the module. The registered color names are
  a `SwapCell` from SubEtha's `subetha-core`, which any runspace reads without
  waiting while `Register-ColorName` and `Unregister-ColorName` swap in a new
  table; the detected color support is one atomic; and the values built once,
  such as the 256-color table, are set with a compare-and-swap. A lock guarded
  the names and the detected support before.
- `Export-ColorProfile` writes a list of one item, such as a style's single
  named gradient, as that item (`"Gradient": "Sunset"`), as PSWriteColorEX
  writes it. It wrote a one-item array before.

### Fixed

- `-Style` refuses what PSWriteColorEX's refuses, with the same error:
  `$null`, a `$null` entry, and an entry that is not a string, an integer or
  an array, such as 1.5 or a hashtable. It took any value before.
- A style holding a value outside its parameter's set, such as a
  `GradientSpace` of `Wavy`, makes `Write-ColorEX -StyleProfile` and `-Default`
  write PowerShell's parameter-check error and then the text, with that setting
  left out. The value was ignored before.
- Importing the module again, in the same runspace or another, keeps the
  profiles added since and puts back the built-in ones and the default style,
  as PSWriteColorEX does. Each import replaced the whole profile table before,
  which dropped the profiles a script had added, in every runspace.

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

[1.2.1]: https://www.powershellgallery.com/packages/PWRSWriteColorEX/1.2.1
[1.2.0]: https://www.powershellgallery.com/packages/PWRSWriteColorEX/1.2.0
