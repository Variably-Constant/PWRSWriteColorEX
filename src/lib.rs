//! PWRSWriteColorEX: the commands of [PSWriteColorEX](https://github.com/MarkusMcNugen/PSWriteColorEX)
//! written in Rust with [PWRS](https://github.com/Variably-Constant/PWRS), with the same names,
//! parameters and aliases, and the PSColorStyle class in C# beside them.
//!
//! The `module` feature, on by default, exports the PowerShell module. Without it the crate is a
//! library of the module's color code, for another PWRS module to link.

// Without `module` nothing exports the cmdlets, so the code only they reach goes unused.
#![cfg_attr(not(feature = "module"), allow(dead_code))]

#[cfg(feature = "module")]
use pwrs::prelude::*;

// A cmdlet's doc comments are its PowerShell help, which writes types such as [PSColorStyle] as
// PowerShell does, so rustdoc does not read the modules holding cmdlets for links.
mod arith;
pub mod colors;
#[allow(rustdoc::broken_intra_doc_links)]
pub mod convert;
#[allow(rustdoc::broken_intra_doc_links)]
pub mod detect;
pub mod ease;
mod format;
pub mod forms;
#[allow(rustdoc::broken_intra_doc_links)]
pub mod gradient;
mod helpers;
mod host;
mod log;
#[allow(rustdoc::broken_intra_doc_links)]
pub mod names;
pub mod oklab;
mod profiles;
pub mod runs;
mod set_once;
pub mod spaces;
#[allow(rustdoc::broken_intra_doc_links)]
pub mod style;
#[allow(rustdoc::broken_intra_doc_links)]
pub mod width;
mod width_sets;
#[allow(rustdoc::broken_intra_doc_links)]
pub mod write;

#[cfg(feature = "module")]
use convert::{
    ConvertHexToRGB, ConvertRGBToANSI4, ConvertRGBToANSI8, GetColorContrast, GetColorTableWithRGB, GetLighterANSI8Color, GetLighterColorName,
    GetLighterRGBColor,
};
#[cfg(feature = "module")]
use detect::TestAnsiSupport;
#[cfg(feature = "module")]
use format::FormatColorEX;
#[cfg(feature = "module")]
use gradient::GetGradientColor;
#[cfg(feature = "module")]
use helpers::{WriteColorCritical, WriteColorDebug, WriteColorError, WriteColorInfo, WriteColorSuccess, WriteColorWarning};
#[cfg(feature = "module")]
use names::{
    ExportColorProfileName, FormatColorEXBackGroundColor, FormatColorEXBackGroundGradient, FormatColorEXColor, FormatColorEXGradient,
    FormatColorEXUnderlineColor, GetColorContrastBackGroundColor, GetColorContrastColor, GetColorProfilesName, GetGradientColorGradient, NewColorStyleBackgroundColor, NewColorStyleBackgroundGradient, NewColorStyleForegroundColor,
    NewColorStyleGradient, NewColorStyleUnderlineColor, RegisterColorName, RegisterColorNameColor, RemoveColorProfileName, SetColorDefaultBackgroundColor,
    SetColorDefaultForegroundColor, ShowColorTable, ShowColorTableName, UnregisterColorName, UnregisterColorNameName, WriteColorEXBackGroundColor,
    WriteColorEXBackGroundGradient, WriteColorEXColor, WriteColorEXGradient, WriteColorEXUnderlineColor,
};
#[cfg(feature = "module")]
use profiles::{ExportColorProfile, ImportColorProfile, RemoveColorProfile};
#[cfg(feature = "module")]
use style::{GetColorProfiles, NewColorStyle, SetColorDefault};
#[cfg(feature = "module")]
use width::MeasureDisplayWidth;
#[cfg(feature = "module")]
use write::WriteColorEX;

/// Detects the terminal's color support again on the first write after each import, and forgets
/// the registered color names, as importing PSWriteColorEX does. The built-in profiles and
/// [PSColorStyle] are set up by the C# import hook in src/csharp/ModuleHooks.cs.
#[cfg(feature = "module")]
#[on_import]
fn initialize() -> PsResult<()> {
    detect::reset_session_support();
    colors::clear_registered();
    Ok(())
}

#[cfg(feature = "module")]
pwrs::export_module! {
    name: "PWRSWriteColorEX",
    cmdlets: [
        WriteColorEX,
        WriteColorError,
        WriteColorWarning,
        WriteColorInfo,
        WriteColorSuccess,
        WriteColorCritical,
        WriteColorDebug,
        SetColorDefault,
        GetColorProfiles,
        NewColorStyle,
        TestAnsiSupport,
        ConvertHexToRGB,
        ConvertRGBToANSI8,
        ConvertRGBToANSI4,
        GetColorTableWithRGB,
        MeasureDisplayWidth,
        GetLighterRGBColor,
        GetLighterColorName,
        GetLighterANSI8Color,
        FormatColorEX,
        ShowColorTable,
        RegisterColorName,
        UnregisterColorName,
        ExportColorProfile,
        ImportColorProfile,
        RemoveColorProfile,
        GetGradientColor,
        GetColorContrast,
    ],
    completers: [
        WriteColorEXColor,
        WriteColorEXBackGroundColor,
        WriteColorEXUnderlineColor,
        WriteColorEXGradient,
        WriteColorEXBackGroundGradient,
        FormatColorEXColor,
        FormatColorEXBackGroundColor,
        FormatColorEXUnderlineColor,
        FormatColorEXGradient,
        FormatColorEXBackGroundGradient,
        NewColorStyleForegroundColor,
        NewColorStyleBackgroundColor,
        NewColorStyleUnderlineColor,
        NewColorStyleGradient,
        NewColorStyleBackgroundGradient,
        GetGradientColorGradient,
        SetColorDefaultForegroundColor,
        SetColorDefaultBackgroundColor,
        RegisterColorNameColor,
        ShowColorTableName,
        GetColorContrastColor,
        GetColorContrastBackGroundColor,
        UnregisterColorNameName,
        GetColorProfilesName,
        ExportColorProfileName,
        RemoveColorProfileName,
    ],
    on_import: initialize,
}
