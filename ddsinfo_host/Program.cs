using System;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.Loader;

namespace NorthStar.DdsInfoHost
{
    internal static class Program
    {
        private const string LibraryEnv = "NORTHSTAR_DDSINFO_LIBRARY";

        private static int Main(string[] args)
        {
            string libraryPath;
            string[] forwarded;
            if (!TryResolveLibrary(args ?? Array.Empty<string>(), out libraryPath, out forwarded))
                return 2;

            try
            {
                Assembly assembly = AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.GetFullPath(libraryPath));
                Type appType = assembly.GetType("ddsinfo.DdsInfoApplication", throwOnError: false);
                if (appType == null)
                {
                    Console.Error.WriteLine("[ERROR] DDS info application type was not found in: {0}", libraryPath);
                    return 2;
                }

                MethodInfo run = appType.GetMethod("Run", BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static);
                if (run == null)
                {
                    Console.Error.WriteLine("[ERROR] DDS info Run(string[]) entry point was not found in: {0}", libraryPath);
                    return 2;
                }

                object result = run.Invoke(null, new object[] { forwarded });
                return result is int code ? code : 0;
            }
            catch (TargetInvocationException ex)
            {
                Exception inner = ex.InnerException ?? ex;
                Console.Error.WriteLine("[ERROR] DDS info execution failed: {0}", inner.Message);
                return 1;
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine("[ERROR] DDS info host failed: {0}", ex.Message);
                return 1;
            }
        }

        private static bool TryResolveLibrary(string[] args, out string libraryPath, out string[] forwarded)
        {
            libraryPath = Environment.GetEnvironmentVariable(LibraryEnv) ?? string.Empty;
            if (string.IsNullOrWhiteSpace(libraryPath))
            {
                string sibling = Path.Combine(AppContext.BaseDirectory, "ddsinfo.dll");
                if (File.Exists(sibling))
                    libraryPath = sibling;
            }
            forwarded = args;

            for (int i = 0; i < args.Length; ++i)
            {
                string arg = args[i];
                if (arg == "--")
                {
                    forwarded = args.Skip(i + 1).ToArray();
                    return ValidateLibrary(libraryPath);
                }

                if (arg == "--library" || arg == "--ddsinfo-library")
                {
                    if (i + 1 >= args.Length)
                    {
                        Console.Error.WriteLine("[ERROR] {0} requires a path value.", arg);
                        forwarded = Array.Empty<string>();
                        return false;
                    }

                    libraryPath = args[i + 1];
                    forwarded = args.Take(i).Concat(args.Skip(i + 2)).ToArray();
                    return ValidateLibrary(libraryPath);
                }

                const string libraryEq = "--library=";
                const string ddsInfoLibraryEq = "--ddsinfo-library=";
                if (arg.StartsWith(libraryEq, StringComparison.Ordinal))
                {
                    libraryPath = arg.Substring(libraryEq.Length);
                    forwarded = args.Take(i).Concat(args.Skip(i + 1)).ToArray();
                    return ValidateLibrary(libraryPath);
                }
                if (arg.StartsWith(ddsInfoLibraryEq, StringComparison.Ordinal))
                {
                    libraryPath = arg.Substring(ddsInfoLibraryEq.Length);
                    forwarded = args.Take(i).Concat(args.Skip(i + 1)).ToArray();
                    return ValidateLibrary(libraryPath);
                }
            }

            return ValidateLibrary(libraryPath);
        }

        private static bool ValidateLibrary(string libraryPath)
        {
            if (string.IsNullOrWhiteSpace(libraryPath))
            {
                Console.Error.WriteLine("[ERROR] DDS info library path was not supplied. Use --library <path> or set {0}.", LibraryEnv);
                return false;
            }
            if (!File.Exists(libraryPath))
            {
                Console.Error.WriteLine("[ERROR] DDS info library does not exist: {0}", libraryPath);
                return false;
            }
            return true;
        }
    }
}
