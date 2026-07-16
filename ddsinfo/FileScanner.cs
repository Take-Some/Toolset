//
// File: FileScanner.cs
// Description: Directory scanner with recoverable errors.
//

using System;
using System.Collections.Generic;
using System.IO;

namespace ddsinfo
{
    internal static class FileScanner
    {
        public static IEnumerable<string> EnumerateFiles(string root, string filter, bool recurse)
        {
            Stack<string> pending = new Stack<string>();
            pending.Push(root);

            while (pending.Count != 0)
            {
                string directory = pending.Pop();

                string[] files = SafeGetFiles(directory, filter);
                for (int i = 0; i < files.Length; ++i)
                    yield return files[i];

                if (!recurse)
                    continue;

                string[] dirs = SafeGetDirectories(directory);
                for (int i = 0; i < dirs.Length; ++i)
                    pending.Push(dirs[i]);
            }
        }

        private static string[] SafeGetFiles(string directory, string filter)
        {
            try
            {
                return Directory.GetFiles(directory, filter);
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine("Failed to enumerate files in {0}: {1}", directory, ex.Message);
                return new string[0];
            }
        }

        private static string[] SafeGetDirectories(string directory)
        {
            try
            {
                return Directory.GetDirectories(directory);
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine("Failed to enumerate directories in {0}: {1}", directory, ex.Message);
                return new string[0];
            }
        }
    }
}
