//
// File: ConsoleReporter.cs
// Description: Output formatting.
//

using System;
using System.Text;

namespace ddsinfo
{
    internal static class ConsoleReporter
    {
        public static void Write(DdsInfoRecord record, AppOptions options)
        {
            StringBuilder line = new StringBuilder();
            line.Append(record.Path);

            if (options.Dimensions)
            {
                line.Append(' ');
                line.Append(record.Width);
                line.Append(" x ");
                line.Append(record.Height);
                line.Append(" x ");
                line.Append(record.Depth);
            }

            if (options.Compression)
            {
                line.Append(' ');
                line.Append(record.Compression);
            }

            if (options.Format)
            {
                line.Append(" format=");
                line.Append(record.Format);
            }

            if (options.MipMaps)
            {
                line.Append(" mips=");
                line.Append(record.MipMapCount);
            }

            Console.WriteLine(line.ToString());
        }
    }
}
