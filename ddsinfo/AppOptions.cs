//
// File: AppOptions.cs
// Description: Dependency-free command line parser for ddsinfo.
//

using System;
using System.Collections.Generic;

namespace ddsinfo
{
    internal sealed class AppOptions
    {
        public const string DefaultFilter = "*.dds";

        private readonly List<string> _paths;
        private readonly List<string> _errors;

        private bool _showHelp;
        private bool _showVersion;
        private bool _showAcceptedInputs;
        private bool _showDoctor;
        private bool _recurse;
        private bool _dimensions;
        private bool _compression;
        private bool _format;
        private bool _mipmaps;
        private bool _strict;
        private string _filter;

        public AppOptions()
        {
            _paths = new List<string>();
            _errors = new List<string>();
            _filter = DefaultFilter;
        }

        public bool ShowHelp { get { return _showHelp; } }
        public bool ShowVersion { get { return _showVersion; } }
        public bool ShowAcceptedInputs { get { return _showAcceptedInputs; } }
        public bool ShowDoctor { get { return _showDoctor; } }
        public bool Recurse { get { return _recurse; } }
        public bool Dimensions { get { return _dimensions; } }
        public bool Compression { get { return _compression; } }
        public bool Format { get { return _format; } }
        public bool MipMaps { get { return _mipmaps; } }
        public bool Strict { get { return _strict; } }
        public string Filter { get { return _filter; } }
        public IList<string> Paths { get { return _paths.AsReadOnly(); } }
        public IList<string> Errors { get { return _errors.AsReadOnly(); } }

        public bool HasErrors { get { return _errors.Count != 0; } }

        public bool AnyOutputFieldSelected
        {
            get { return _dimensions || _compression || _format || _mipmaps; }
        }

        public static AppOptions Parse(string[] args)
        {
            AppOptions options = new AppOptions();
            if (args == null)
                return options;

            for (int i = 0; i < args.Length; ++i)
            {
                string arg = args[i];
                if (arg == null || arg.Length == 0)
                    continue;

                if (arg == "--")
                {
                    AddRemainingPaths(options, args, i + 1);
                    break;
                }

                if (arg.StartsWith("--"))
                {
                    ParseLongOption(options, args, ref i, arg);
                    continue;
                }

                if (arg.Length > 1 && arg[0] == '-')
                {
                    ParseShortOptions(options, args, ref i, arg);
                    continue;
                }

                if (EqualsOption(arg, "help"))
                    options._showHelp = true;
                else if (EqualsOption(arg, "version"))
                    options._showVersion = true;
                else if (EqualsOption(arg, "accepted-inputs"))
                    options._showAcceptedInputs = true;
                else if (EqualsOption(arg, "doctor"))
                    options._showDoctor = true;
                else
                    options._paths.Add(arg);
            }

            if (!options.AnyOutputFieldSelected)
            {
                // A safer production default than the legacy "filename only" output.
                options._dimensions = true;
                options._compression = true;
            }

            return options;
        }

        private static void AddRemainingPaths(AppOptions options, string[] args, int start)
        {
            for (int i = start; i < args.Length; ++i)
                options._paths.Add(args[i]);
        }

        private static void ParseLongOption(AppOptions options, string[] args, ref int index, string arg)
        {
            string name = arg.Substring(2);
            string value = null;
            int eq = name.IndexOf('=');
            if (eq >= 0)
            {
                value = name.Substring(eq + 1);
                name = name.Substring(0, eq);
            }

            if (EqualsOption(name, "help"))
                options._showHelp = true;
            else if (EqualsOption(name, "version"))
                options._showVersion = true;
            else if (EqualsOption(name, "accepted-inputs"))
                options._showAcceptedInputs = true;
            else if (EqualsOption(name, "doctor"))
                options._showDoctor = true;
            else if (EqualsOption(name, "recurse"))
                options._recurse = true;
            else if (EqualsOption(name, "dimensions"))
                options._dimensions = true;
            else if (EqualsOption(name, "compression"))
                options._compression = true;
            else if (EqualsOption(name, "format"))
                options._format = true;
            else if (EqualsOption(name, "mipmaps"))
                options._mipmaps = true;
            else if (EqualsOption(name, "strict"))
                options._strict = true;
            else if (EqualsOption(name, "filter"))
                options._filter = RequireOptionValue(options, args, ref index, name, value);
            else
                options._errors.Add("Unknown option: --" + name);
        }

        private static void ParseShortOptions(AppOptions options, string[] args, ref int index, string arg)
        {
            for (int pos = 1; pos < arg.Length; ++pos)
            {
                char ch = arg[pos];
                switch (ch)
                {
                    case 'h': options._showHelp = true; break;
                    case 'v': options._showVersion = true; break;
                    case 'r': options._recurse = true; break;
                    case 'd': options._dimensions = true; break;
                    case 'c': options._compression = true; break;
                    case 'm': options._mipmaps = true; break;
                    case 's': options._strict = true; break;
                    case 'F': options._format = true; break;
                    case 'f':
                        string inline = null;
                        if (pos + 1 < arg.Length)
                        {
                            inline = arg.Substring(pos + 1);
                            pos = arg.Length;
                        }
                        options._filter = RequireOptionValue(options, args, ref index, "f", inline);
                        break;
                    default:
                        options._errors.Add("Unknown option: -" + ch);
                        break;
                }
            }
        }

        private static string RequireOptionValue(AppOptions options, string[] args, ref int index, string optionName, string inlineValue)
        {
            if (!IsNullOrEmpty(inlineValue))
                return inlineValue;

            if (index + 1 >= args.Length)
            {
                options._errors.Add("Option requires a value: " + optionName);
                return DefaultFilter;
            }

            string next = args[index + 1];
            if (IsNullOrEmpty(next))
            {
                options._errors.Add("Option requires a non-empty value: " + optionName);
                return DefaultFilter;
            }

            index += 1;
            return next;
        }

        private static bool EqualsOption(string lhs, string rhs)
        {
            return string.Equals(lhs, rhs, StringComparison.OrdinalIgnoreCase);
        }

        private static bool IsNullOrEmpty(string value)
        {
            return value == null || value.Length == 0;
        }
    }
}
