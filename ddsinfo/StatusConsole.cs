using System;

namespace ddsinfo
{
    internal static class StatusConsole
    {
        public static void Info(string message) { Write(Console.Out, "INFO", message, ConsoleColor.Blue); }
        public static void Ok(string message) { Write(Console.Out, "OK", message, ConsoleColor.Green); }
        public static void Warn(string message) { Write(Console.Out, "WARN", message, ConsoleColor.Yellow); }
        public static void Error(string message) { Write(Console.Error, "ERROR", message, ConsoleColor.Red); }

        private static void Write(System.IO.TextWriter writer, string tag, string message, ConsoleColor color)
        {
            bool canColor = !Console.IsOutputRedirected && !NoColorRequested();
            if (!canColor)
            {
                writer.WriteLine("[" + tag + "] " + message);
                return;
            }
            ConsoleColor previous = Console.ForegroundColor;
            try
            {
                Console.ForegroundColor = ConsoleColor.DarkGray;
                writer.Write("[");
                Console.ForegroundColor = color;
                writer.Write(tag);
                Console.ForegroundColor = ConsoleColor.DarkGray;
                writer.Write("] ");
                Console.ForegroundColor = previous;
                writer.WriteLine(message);
            }
            finally
            {
                Console.ForegroundColor = previous;
            }
        }

        private static bool NoColorRequested()
        {
            return Environment.GetEnvironmentVariable("NO_COLOR") != null ||
                   IsTruthy(Environment.GetEnvironmentVariable("NORTHSTAR_NO_COLOR")) ||
                   IsNever(Environment.GetEnvironmentVariable("NORTHSTAR_COLOR"));
        }

        private static bool IsTruthy(string value)
        {
            if (string.IsNullOrWhiteSpace(value)) return false;
            string normalized = value.Trim().ToLowerInvariant();
            return normalized != "0" && normalized != "false" && normalized != "no" && normalized != "off";
        }

        private static bool IsNever(string value)
        {
            if (string.IsNullOrWhiteSpace(value)) return false;
            string normalized = value.Trim().ToLowerInvariant();
            return normalized == "never" || normalized == "plain" || normalized == "0" || normalized == "false" || normalized == "no" || normalized == "off";
        }
    }
}
