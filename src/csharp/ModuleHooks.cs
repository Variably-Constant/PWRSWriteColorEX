// Makes [PSColorStyle] resolve in the session and starts the built-in profiles when the module is
// imported, and takes the name away when the module is removed. PowerShell 7 loads the module's
// assembly in a load context of its own, where the type resolver does not find a class by its
// name, so the class is registered as a type accelerator, as PSWriteColorEX registers its own.

using System;
using System.Collections;
using System.Management.Automation;

public sealed class PSColorStyleRegistration : IModuleAssemblyInitializer, IModuleAssemblyCleanup
{
    private const string AcceleratorName = "PSColorStyle";

    // Each import starts a new profile table with the built-in profiles, as importing
    // PSWriteColorEX defines its class anew
    public void OnImport()
    {
        PSColorStyle.Profiles = new Hashtable(StringComparer.CurrentCultureIgnoreCase);
        PSColorStyle.InitializeDefaultProfiles();
        var accelerators = Accelerators();
        var add = accelerators?.GetMethod("Add", new[] { typeof(string), typeof(Type) });
        add?.Invoke(null, new object[] { AcceleratorName, typeof(PSColorStyle) });
    }

    // Only this module's own registration is taken away
    public void OnRemove(PSModuleInfo psModuleInfo)
    {
        var accelerators = Accelerators();
        var registered = accelerators?.GetProperty("Get")?.GetValue(null) as IDictionary;
        if (registered == null || !ReferenceEquals(registered[AcceleratorName], typeof(PSColorStyle)))
        {
            return;
        }
        var remove = accelerators?.GetMethod("Remove", new[] { typeof(string) });
        remove?.Invoke(null, new object[] { AcceleratorName });
    }

    private static Type? Accelerators()
    {
        return typeof(PSObject).Assembly.GetType("System.Management.Automation.TypeAccelerators");
    }
}
