//
// File: DdsHeaderReader.cs
// Description: Safe DDS header parser; no DevIL/Tao dependency.
//

using System;
using System.IO;
using System.Text;

namespace ddsinfo
{
    internal static class DdsHeaderReader
    {
        private const uint DdsMagic = 0x20534444; // "DDS "
        private const uint HeaderSize = 124;
        private const uint PixelFormatSize = 32;

        private const uint DdsdDepth = 0x00800000;
        private const uint DdsdMipMapCount = 0x00020000;

        private const uint DdpfAlphaPixels = 0x00000001;
        private const uint DdpfFourCc = 0x00000004;
        private const uint DdpfRgb = 0x00000040;
        private const uint DdpfLuminance = 0x00020000;

        public static DdsInfoRecord Read(string path)
        {
            if (path == null || path.Length == 0)
                throw new ArgumentException("empty path");

            FileInfo file = new FileInfo(path);
            if (!file.Exists)
                throw new FileNotFoundException("file not found", path);
            if (file.Length < 128)
                throw new InvalidDataException("file is too small to be a DDS image");

            using (FileStream stream = File.Open(path, FileMode.Open, FileAccess.Read, FileShare.Read))
            using (BinaryReader reader = new BinaryReader(stream))
            {
                uint magic = reader.ReadUInt32();
                if (magic != DdsMagic)
                    throw new InvalidDataException("invalid DDS magic");

                uint headerSize = reader.ReadUInt32();
                if (headerSize != HeaderSize)
                    throw new InvalidDataException("invalid DDS header size");

                uint flags = reader.ReadUInt32();
                int height = CheckedDimension(reader.ReadUInt32(), "height");
                int width = CheckedDimension(reader.ReadUInt32(), "width");

                reader.ReadUInt32(); // pitchOrLinearSize
                uint rawDepth = reader.ReadUInt32();
                uint rawMipMapCount = reader.ReadUInt32();

                SkipBytes(reader, 44); // reserved1[11]

                uint pfSize = reader.ReadUInt32();
                if (pfSize != PixelFormatSize)
                    throw new InvalidDataException("invalid DDS pixel format size");

                uint pfFlags = reader.ReadUInt32();
                uint fourCc = reader.ReadUInt32();
                uint rgbBitCount = reader.ReadUInt32();
                uint rMask = reader.ReadUInt32();
                uint gMask = reader.ReadUInt32();
                uint bMask = reader.ReadUInt32();
                uint aMask = reader.ReadUInt32();

                reader.ReadUInt32(); // caps
                reader.ReadUInt32(); // caps2
                reader.ReadUInt32(); // caps3
                reader.ReadUInt32(); // caps4
                reader.ReadUInt32(); // reserved2

                int depth = ((flags & DdsdDepth) != 0 && rawDepth > 0) ? CheckedDimension(rawDepth, "depth") : 1;
                int mipMapCount = ((flags & DdsdMipMapCount) != 0 && rawMipMapCount > 0) ? CheckedDimension(rawMipMapCount, "mip map count") : 1;

                bool dx10 = false;
                string compression;
                string format;

                if ((pfFlags & DdpfFourCc) != 0)
                {
                    string fourCcText = FourCcToString(fourCc);
                    if (fourCcText == "DX10")
                    {
                        if (stream.Length < 148)
                            throw new InvalidDataException("DDS DX10 header is truncated");

                        uint dxgiFormat = reader.ReadUInt32();
                        reader.ReadUInt32(); // resourceDimension
                        reader.ReadUInt32(); // miscFlag
                        reader.ReadUInt32(); // arraySize
                        reader.ReadUInt32(); // miscFlags2

                        dx10 = true;
                        compression = DxgiCompressionName(dxgiFormat);
                        format = DxgiFormatName(dxgiFormat);
                    }
                    else
                    {
                        compression = FourCcCompressionName(fourCcText);
                        format = fourCcText;
                    }
                }
                else
                {
                    compression = "NONE";
                    format = PixelFormatName(pfFlags, rgbBitCount, rMask, gMask, bMask, aMask);
                }

                return new DdsInfoRecord(path, width, height, depth, mipMapCount, compression, format, dx10);
            }
        }

        private static int CheckedDimension(uint value, string name)
        {
            if (value == 0)
                throw new InvalidDataException("DDS " + name + " is zero");
            if (value > Int32.MaxValue)
                throw new InvalidDataException("DDS " + name + " is too large");
            return (int)value;
        }

        private static void SkipBytes(BinaryReader reader, int count)
        {
            byte[] skipped = reader.ReadBytes(count);
            if (skipped.Length != count)
                throw new EndOfStreamException("DDS header is truncated");
        }

        private static string FourCcToString(uint fourCc)
        {
            byte[] bytes = new byte[]
            {
                (byte)(fourCc & 0xFF),
                (byte)((fourCc >> 8) & 0xFF),
                (byte)((fourCc >> 16) & 0xFF),
                (byte)((fourCc >> 24) & 0xFF)
            };
            return Encoding.ASCII.GetString(bytes);
        }

        private static string FourCcCompressionName(string fourCc)
        {
            if (fourCc == "DXT1") return "DXT1";
            if (fourCc == "DXT2") return "DXT2";
            if (fourCc == "DXT3") return "DXT3";
            if (fourCc == "DXT4") return "DXT4";
            if (fourCc == "DXT5") return "DXT5";
            if (fourCc == "ATI1" || fourCc == "BC4U" || fourCc == "BC4S") return "BC4";
            if (fourCc == "ATI2" || fourCc == "BC5U" || fourCc == "BC5S") return "BC5";
            return "UNKNOWN";
        }

        private static string DxgiCompressionName(uint dxgiFormat)
        {
            if (dxgiFormat >= 70 && dxgiFormat <= 72) return "BC1";
            if (dxgiFormat >= 73 && dxgiFormat <= 75) return "BC2";
            if (dxgiFormat >= 76 && dxgiFormat <= 78) return "BC3";
            if (dxgiFormat >= 79 && dxgiFormat <= 81) return "BC4";
            if (dxgiFormat >= 82 && dxgiFormat <= 84) return "BC5";
            if (dxgiFormat >= 94 && dxgiFormat <= 96) return "BC6H";
            if (dxgiFormat >= 97 && dxgiFormat <= 99) return "BC7";
            return "NONE";
        }

        private static string DxgiFormatName(uint dxgiFormat)
        {
            switch (dxgiFormat)
            {
                case 70: return "DX10 BC1_TYPELESS";
                case 71: return "DX10 BC1_UNORM";
                case 72: return "DX10 BC1_UNORM_SRGB";
                case 73: return "DX10 BC2_TYPELESS";
                case 74: return "DX10 BC2_UNORM";
                case 75: return "DX10 BC2_UNORM_SRGB";
                case 76: return "DX10 BC3_TYPELESS";
                case 77: return "DX10 BC3_UNORM";
                case 78: return "DX10 BC3_UNORM_SRGB";
                case 79: return "DX10 BC4_TYPELESS";
                case 80: return "DX10 BC4_UNORM";
                case 81: return "DX10 BC4_SNORM";
                case 82: return "DX10 BC5_TYPELESS";
                case 83: return "DX10 BC5_UNORM";
                case 84: return "DX10 BC5_SNORM";
                case 85: return "DX10 B5G6R5_UNORM";
                case 86: return "DX10 B5G5R5A1_UNORM";
                case 87: return "DX10 B8G8R8A8_UNORM";
                case 88: return "DX10 B8G8R8X8_UNORM";
                case 89: return "DX10 R10G10B10_XR_BIAS_A2_UNORM";
                case 90: return "DX10 B8G8R8A8_TYPELESS";
                case 91: return "DX10 B8G8R8A8_UNORM_SRGB";
                case 92: return "DX10 B8G8R8X8_TYPELESS";
                case 93: return "DX10 B8G8R8X8_UNORM_SRGB";
                case 94: return "DX10 BC6H_TYPELESS";
                case 95: return "DX10 BC6H_UF16";
                case 96: return "DX10 BC6H_SF16";
                case 97: return "DX10 BC7_TYPELESS";
                case 98: return "DX10 BC7_UNORM";
                case 99: return "DX10 BC7_UNORM_SRGB";
                default: return "DX10 DXGI_FORMAT_" + dxgiFormat.ToString();
            }
        }

        private static string PixelFormatName(uint flags, uint bits, uint rMask, uint gMask, uint bMask, uint aMask)
        {
            string kind;
            if ((flags & DdpfRgb) != 0)
                kind = "RGB";
            else if ((flags & DdpfLuminance) != 0)
                kind = "LUMINANCE";
            else
                kind = "UNCOMPRESSED";

            bool alpha = (flags & DdpfAlphaPixels) != 0;
            return string.Format(
                "{0}{1} {2}bpp R=0x{3:X8} G=0x{4:X8} B=0x{5:X8} A=0x{6:X8}",
                kind,
                alpha ? "+A" : string.Empty,
                bits,
                rMask,
                gMask,
                bMask,
                aMask);
        }
    }
}
