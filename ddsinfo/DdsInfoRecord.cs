//
// File: DdsInfoRecord.cs
// Description: Immutable DDS metadata record.
//

namespace ddsinfo
{
    internal sealed class DdsInfoRecord
    {
        private readonly string _path;
        private readonly int _width;
        private readonly int _height;
        private readonly int _depth;
        private readonly int _mipMapCount;
        private readonly string _compression;
        private readonly string _format;
        private readonly bool _dx10Header;

        public DdsInfoRecord(
            string path,
            int width,
            int height,
            int depth,
            int mipMapCount,
            string compression,
            string format,
            bool dx10Header)
        {
            _path = path;
            _width = width;
            _height = height;
            _depth = depth;
            _mipMapCount = mipMapCount;
            _compression = compression;
            _format = format;
            _dx10Header = dx10Header;
        }

        public string Path { get { return _path; } }
        public int Width { get { return _width; } }
        public int Height { get { return _height; } }
        public int Depth { get { return _depth; } }
        public int MipMapCount { get { return _mipMapCount; } }
        public string Compression { get { return _compression; } }
        public string Format { get { return _format; } }
        public bool Dx10Header { get { return _dx10Header; } }
    }
}
