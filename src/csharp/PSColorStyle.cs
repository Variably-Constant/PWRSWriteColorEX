// A reusable style for Write-ColorEX: colors, text styles and layout, with the named profiles
// the Write-Color* helpers read. The same class as PSWriteColorEX's, so scripts written for one
// module run with the other: the static properties Profiles and Default, the settable properties,
// and the methods. It is C# beside the Rust cmdlets because a PWRS class defined in Rust has no
// static properties.

using System;
using System.Collections;
using System.Management.Automation;

public class PSColorStyle
{
    public string? Name { get; set; }
    public object? ForegroundColor { get; set; }
    public object? BackgroundColor { get; set; }
    public object?[]? Gradient { get; set; }
    public string?[]? Style { get; set; }
    public int StartTab { get; set; }
    public int StartSpaces { get; set; }
    public int LinesBefore { get; set; }
    public int LinesAfter { get; set; }
    public bool Bold { get; set; }
    public bool Italic { get; set; }
    public bool Underline { get; set; }
    public bool Blink { get; set; }
    public bool Faint { get; set; }
    public bool CrossedOut { get; set; }
    public bool DoubleUnderline { get; set; }
    public bool Overline { get; set; }
    public bool ShowTime { get; set; }
    public bool NoNewLine { get; set; }
    public bool HorizontalCenter { get; set; }
    public int AutoPad { get; set; }
    public bool PadLeft { get; set; }
    public char PadChar { get; set; }
    public object?[]? BackgroundGradient { get; set; }
    public string? GradientSpace { get; set; }
    public bool Reverse { get; set; }
    public object? UnderlineColor { get; set; }
    public string? UnderlineStyle { get; set; }
    public bool PadCenter { get; set; }
    public bool Truncate { get; set; }
    public bool Wrap { get; set; }

    // The style -Default applies
    public static PSColorStyle? Default { get; set; }

    // Named styles, with keys compared without regard to case as a PowerShell hashtable's are
    public static Hashtable? Profiles { get; set; } = new Hashtable(StringComparer.CurrentCultureIgnoreCase);

    public PSColorStyle()
    {
        Initialize("Custom", "Gray", null);
    }

    public PSColorStyle(string? name)
    {
        Initialize(name, "Gray", null);
    }

    public PSColorStyle(string? name, object? foreground, object? background)
    {
        Initialize(name, foreground, background);
    }

    private void Initialize(string? name, object? foreground, object? background)
    {
        Name = name;
        ForegroundColor = Unwrap(foreground);
        BackgroundColor = Unwrap(background);
        Gradient = null;
        Style = new string[0];
        PadChar = ' ';
        BackgroundGradient = null;
        GradientSpace = string.Empty;
        UnderlineColor = null;
        UnderlineStyle = string.Empty;
    }

    public void SetAsDefault()
    {
        Default = this;
    }

    public void AddToProfiles()
    {
        if (Profiles == null)
        {
            Profiles = new Hashtable(StringComparer.CurrentCultureIgnoreCase);
        }
        Profiles[Name ?? string.Empty] = this;
    }

    public static PSColorStyle? GetProfile(string? name)
    {
        if (name == null || Profiles == null)
        {
            return null;
        }
        return Unwrap(Profiles[name]) as PSColorStyle;
    }

    // The built-in profiles
    public static void InitializeDefaultProfiles()
    {
        var defaultProfile = new PSColorStyle("Default", "Gray", null);
        Default = defaultProfile;
        defaultProfile.AddToProfiles();

        var errorProfile = new PSColorStyle("Error", "Red", null);
        errorProfile.Bold = true;
        errorProfile.AddToProfiles();

        new PSColorStyle("Warning", "Yellow", null).AddToProfiles();
        new PSColorStyle("Info", "Cyan", null).AddToProfiles();
        new PSColorStyle("Success", "Green", null).AddToProfiles();

        var criticalProfile = new PSColorStyle("Critical", "White", "DarkRed");
        criticalProfile.Bold = true;
        criticalProfile.Blink = true;
        criticalProfile.AddToProfiles();

        var debugProfile = new PSColorStyle("Debug", "DarkGray", null);
        debugProfile.Italic = true;
        debugProfile.AddToProfiles();
    }

    // The Write-ColorEX parameters this style sets, read from its properties
    public Hashtable ToWriteColorParams()
    {
        var parameters = new Hashtable(StringComparer.CurrentCultureIgnoreCase);

        // A gradient replaces the foreground color
        if (Gradient != null && Gradient.Length >= 2)
        {
            parameters["Gradient"] = Gradient;
        }
        else if (LanguagePrimitives.IsTrue(ForegroundColor))
        {
            parameters["Color"] = ForegroundColor;
        }
        // A background gradient replaces the background color
        if (BackgroundGradient != null && BackgroundGradient.Length >= 2)
        {
            parameters["BackGroundGradient"] = BackgroundGradient;
        }
        else if (LanguagePrimitives.IsTrue(BackgroundColor))
        {
            parameters["BackGroundColor"] = BackgroundColor;
        }
        if (!string.IsNullOrEmpty(GradientSpace)) { parameters["GradientSpace"] = GradientSpace; }
        if (Style != null && Style.Length > 0) { parameters["Style"] = Style; }
        if (StartTab > 0) { parameters["StartTab"] = StartTab; }
        if (StartSpaces > 0) { parameters["StartSpaces"] = StartSpaces; }
        if (LinesBefore > 0) { parameters["LinesBefore"] = LinesBefore; }
        if (LinesAfter > 0) { parameters["LinesAfter"] = LinesAfter; }
        if (Bold) { parameters["Bold"] = true; }
        if (Italic) { parameters["Italic"] = true; }
        if (Underline) { parameters["Underline"] = true; }
        if (Blink) { parameters["Blink"] = true; }
        if (Faint) { parameters["Faint"] = true; }
        if (CrossedOut) { parameters["CrossedOut"] = true; }
        if (DoubleUnderline) { parameters["DoubleUnderline"] = true; }
        if (Overline) { parameters["Overline"] = true; }
        if (ShowTime) { parameters["ShowTime"] = true; }
        if (NoNewLine) { parameters["NoNewLine"] = true; }
        if (HorizontalCenter) { parameters["HorizontalCenter"] = true; }
        if (AutoPad > 0) { parameters["AutoPad"] = AutoPad; }
        if (PadLeft) { parameters["PadLeft"] = true; }
        if (PadChar != ' ') { parameters["PadChar"] = PadChar; }
        if (Reverse) { parameters["Reverse"] = true; }
        if (UnderlineColor != null) { parameters["UnderlineColor"] = UnderlineColor; }
        if (!string.IsNullOrEmpty(UnderlineStyle)) { parameters["UnderlineStyle"] = UnderlineStyle; }
        if (PadCenter) { parameters["PadCenter"] = true; }
        if (Truncate) { parameters["Truncate"] = true; }
        if (Wrap) { parameters["Wrap"] = true; }

        return parameters;
    }

    // Does nothing: ToWriteColorParams reads the properties on each call. Kept so scripts that
    // call it after changing a style keep working.
    [Hidden]
    public void InvalidateCache()
    {
    }

    // A copy named with _Copy. Arrays are copied as well, so changing an element of the copy
    // leaves the original alone.
    public PSColorStyle Clone()
    {
        var copy = new PSColorStyle(Name + "_Copy", CopyValue(ForegroundColor), CopyValue(BackgroundColor));
        copy.Gradient = (object?[]?)CopyValue(Gradient);
        copy.Style = (string?[]?)CopyValue(Style);
        copy.StartTab = StartTab;
        copy.StartSpaces = StartSpaces;
        copy.LinesBefore = LinesBefore;
        copy.LinesAfter = LinesAfter;
        copy.Bold = Bold;
        copy.Italic = Italic;
        copy.Underline = Underline;
        copy.Blink = Blink;
        copy.Faint = Faint;
        copy.CrossedOut = CrossedOut;
        copy.DoubleUnderline = DoubleUnderline;
        copy.Overline = Overline;
        copy.ShowTime = ShowTime;
        copy.NoNewLine = NoNewLine;
        copy.HorizontalCenter = HorizontalCenter;
        copy.AutoPad = AutoPad;
        copy.PadLeft = PadLeft;
        copy.PadChar = PadChar;
        copy.BackgroundGradient = (object?[]?)CopyValue(BackgroundGradient);
        copy.GradientSpace = GradientSpace;
        copy.Reverse = Reverse;
        copy.UnderlineColor = CopyValue(UnderlineColor);
        copy.UnderlineStyle = UnderlineStyle;
        copy.PadCenter = PadCenter;
        copy.Truncate = Truncate;
        copy.Wrap = Wrap;
        return copy;
    }

    // A copy of an array, and of the arrays in it such as RGB colors; any other value as it is
    private static object? CopyValue(object? value)
    {
        var array = value as Array;
        if (array == null)
        {
            return value;
        }
        var copy = (Array)array.Clone();
        for (int i = 0; i < copy.Length; i++)
        {
            var inner = copy.GetValue(i) as Array;
            if (inner != null)
            {
                copy.SetValue(inner.Clone(), i);
            }
        }
        return copy;
    }

    // A value as the script gave it, without the PSObject wrapper the engine may put around it
    private static object? Unwrap(object? value)
    {
        var wrapped = value as PSObject;
        return wrapped != null ? wrapped.BaseObject : value;
    }
}
