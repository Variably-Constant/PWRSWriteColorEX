//! PWRSWriteColorEX: the commands of PSWriteColorEX (https://github.com/MarkusMcNugen/PSWriteColorEX)
//! written in Rust with PWRS (https://github.com/Variably-Constant/PWRS), with the same names,
//! parameters and aliases, and the PSColorStyle class in C# beside them.

use pwrs::prelude::*;

mod colors;
mod convert;
mod detect;
mod format;
mod forms;
mod gradient;
mod helpers;
mod host;
mod log;
mod names;
mod oklab;
mod profiles;
mod runs;
mod style;
mod width;
mod width_sets;
mod write;

use convert::{
    ConvertHexToRGB, ConvertRGBToANSI4, ConvertRGBToANSI8, GetColorTableWithRGB, GetLighterANSI8Color, GetLighterColorName,
    GetLighterRGBColor,
};
use detect::TestAnsiSupport;
use format::FormatColorEX;
use helpers::{WriteColorCritical, WriteColorDebug, WriteColorError, WriteColorInfo, WriteColorSuccess, WriteColorWarning};
use names::{
    ExportColorProfileName, FormatColorEXBackGroundColor, FormatColorEXBackGroundGradient, FormatColorEXColor, FormatColorEXGradient,
    FormatColorEXUnderlineColor, GetColorProfilesName, NewColorStyleBackgroundColor, NewColorStyleBackgroundGradient, NewColorStyleForegroundColor,
    NewColorStyleGradient, NewColorStyleUnderlineColor, RegisterColorName, RegisterColorNameColor, RemoveColorProfileName, SetColorDefaultBackgroundColor,
    SetColorDefaultForegroundColor, ShowColorTable, ShowColorTableName, UnregisterColorName, UnregisterColorNameName, WriteColorEXBackGroundColor,
    WriteColorEXBackGroundGradient, WriteColorEXColor, WriteColorEXGradient, WriteColorEXUnderlineColor,
};
use profiles::{ExportColorProfile, ImportColorProfile, RemoveColorProfile};
use style::{GetColorProfiles, NewColorStyle, SetColorDefault};
use width::MeasureDisplayWidth;
use write::WriteColorEX;

/// Detects the terminal's color support again on the first write after each import, and forgets
/// the registered color names, as importing PSWriteColorEX does. The built-in profiles and
/// [PSColorStyle] are set up by the C# import hook in src/csharp/ModuleHooks.cs.
#[on_import]
fn initialize() -> PsResult<()> {
    detect::reset_session_support();
    colors::clear_registered();
    Ok(())
}

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
        SetColorDefaultForegroundColor,
        SetColorDefaultBackgroundColor,
        RegisterColorNameColor,
        ShowColorTableName,
        UnregisterColorNameName,
        GetColorProfilesName,
        ExportColorProfileName,
        RemoveColorProfileName,
    ],
    on_import: initialize,
}
