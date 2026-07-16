//
// File: DdsInfoApplication.cs
// Description: Console application orchestration.
//

using System;
using System.IO;
using System.Reflection;

namespace ddsinfo
{
    public static class DdsInfoApplication
    {
        private const string UsageText =
            "\nddsinfo [options] <paths>\n" +
            "\nddsinfo operates on DDS files specified on the command line, or on DDS files\n" +
            "inside directories specified on the command line.\n" +
            "\noptions:\n" +
            "  --version|-v|version Display version information\n" +
            "  --help|-h|help       Display this usage information\n" +
            "  --accepted-inputs    Print accepted input/output contract\n" +
            "  --doctor             Run a lightweight self-check\n" +
            "  --recurse|-r         Recurse into subdirectories\n" +
            "  --dimensions|-d      Display image dimensions\n" +
            "  --compression|-c     Display block compression family\n" +
            "  --format|-F          Display detailed pixel/DXGI format\n" +
            "  --mipmaps|-m         Display mip map count\n" +
            "  --strict|-s          Return non-zero when any file fails\n" +
            "  --filter|-f <filter> Use wildcard for directory search, default *.dds\n" +
            "\nexamples:\n" +
            "  ddsinfo -r -c -d .\n" +
            "  ddsinfo --format --mipmaps texture.dds\n" +
            "\nnotes:\n" +
            "  This build parses DDS headers directly. It does not require Tao.DevIl,\n" +
            "  RSN.Base, ILMerge, or native DevIL.dll at runtime.\n";

        public static int Run(string[] args)
        {
            AppOptions options = AppOptions.Parse(args);

            if (options.HasErrors)
            {
                PrintOptionErrors(options);
                Usage();
                return 2;
            }

            if (options.ShowVersion)
            {
                Version();
                return 0;
            }

            if (options.ShowAcceptedInputs)
            {
                AcceptedInputs();
                return 0;
            }

            if (options.ShowDoctor)
            {
                Doctor();
                return 0;
            }

            if (options.ShowHelp || options.Paths.Count == 0)
            {
                Usage();
                return 0;
            }

            int processed = 0;
            int failed = 0;

            for (int i = 0; i < options.Paths.Count; ++i)
                ProcessPath(options, options.Paths[i], ref processed, ref failed);

            if (processed == 0)
            {
                Console.Error.WriteLine("No files matched the supplied path/filter set.");
                return 1;
            }

            if (failed != 0 && options.Strict)
                return 1;

            return failed == 0 ? 0 : 1;
        }

        private static void ProcessPath(AppOptions options, string path, ref int processed, ref int failed)
        {
            if (File.Exists(path))
            {
                ProcessFile(options, path, ref processed, ref failed);
                return;
            }

            if (Directory.Exists(path))
            {
                foreach (string file in FileScanner.EnumerateFiles(path, options.Filter, options.Recurse))
                    ProcessFile(options, file, ref processed, ref failed);
                return;
            }

            failed += 1;
            Console.Error.WriteLine("Invalid path specified: {0}", path);
        }

        private static void ProcessFile(AppOptions options, string filename, ref int processed, ref int failed)
        {
            processed += 1;

            try
            {
                DdsInfoRecord record = DdsHeaderReader.Read(filename);
                ConsoleReporter.Write(record, options);
            }
            catch (Exception ex)
            {
                failed += 1;
                Console.Error.WriteLine("{0}: {1}", filename, ex.Message);
            }
        }

        private static void PrintOptionErrors(AppOptions options)
        {
            for (int i = 0; i < options.Errors.Count; ++i)
                Console.Error.WriteLine(options.Errors[i]);
        }

        private static void Usage()
        {
            Console.WriteLine(UsageText);
        }

        private static void Version()
        {
            Assembly assembly = Assembly.GetEntryAssembly();
            Console.WriteLine("northstar-ddsinfo {0}", assembly.GetName().Version.ToString());
        }

        private static void AcceptedInputs()
        {
            StatusConsole.Info("northstar-ddsinfo version=" + Assembly.GetEntryAssembly().GetName().Version.ToString());
            StatusConsole.Info("accepted input files: *.dds files and directories containing DDS files");
            StatusConsole.Info("produced output files: raw stdout DDS metadata lines");
        }

        private static void Doctor()
        {
            StatusConsole.Ok("northstar-ddsinfo doctor passed");
            StatusConsole.Info("version=" + Assembly.GetEntryAssembly().GetName().Version.ToString());
            StatusConsole.Info("DDS header parser=managed");
        }
    }
}
