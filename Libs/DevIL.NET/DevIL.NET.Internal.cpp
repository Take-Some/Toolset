//	DevIL.NET
//	Copyright (c) 2005, Marco Mastropaolo
//	All rights reserved.

//	Redistribution and use in source and binary forms, with or without modification, are permitted provided that the 
//	following conditions are met:

//		* Redistributions of source code must retain the above copyright notice, this list of conditions and the 
//		following disclaimer.
//		* Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the 
//		following disclaimer in the documentation and/or other materials provided with the distribution.
//		* Neither the name of DevIL.NET nor the names of its contributors may be used to endorse or promote products 
//		derived from this software without specific prior written permission.

//	THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, 
//	INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE 
//	DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, 
//	SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR 
//	SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, 
//	WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE 
//	USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#include "DevIL.NET.Internal.h"

namespace DevIL
{
	bool IsNullOrEmpty(System::String __gc* i_pString)
	{
		return i_pString == NULL || i_pString->get_Length() == 0;
	}

	int AbsInt(int i_iValue)
	{
		return i_iValue < 0 ? -i_iValue : i_iValue;
	}

	bool CheckedPixelBufferBytes(int i_iWidth, int i_iHeight, int i_iDepth, int* o_piBytes)
	{
		if (o_piBytes == NULL || i_iWidth <= 0 || i_iHeight <= 0 || i_iDepth <= 0)
		{
			return false;
		}

		if (i_iWidth > INT_MAX / 4)
		{
			return false;
		}

		int iRowBytes = i_iWidth * 4;
		if (i_iHeight > INT_MAX / iRowBytes)
		{
			return false;
		}

		int iPlaneBytes = iRowBytes * i_iHeight;
		if (i_iDepth > INT_MAX / iPlaneBytes)
		{
			return false;
		}

		*o_piBytes = iPlaneBytes * i_iDepth;
		return true;
	}

	unsigned char* BitmapRow(void* i_pScan0, int i_iStride, int i_iY)
	{
		return ((unsigned char*)i_pScan0) + (i_iStride * i_iY);
	}

	DevILErrorCode CopyCurrentDevILImageToBitmap(
		System::Drawing::Imaging::BitmapData __gc* i_pBitmapData,
		int i_iWidth,
		int i_iHeight)
	{
		int iTotalBytes = 0;
		if (!CheckedPixelBufferBytes(i_iWidth, i_iHeight, 1, &iTotalBytes))
		{
			return BAD_DIMENSIONS;
		}

		const int iRowBytes = i_iWidth * 4;
		const int iStride = i_pBitmapData->get_Stride();

		if (iStride == 0 || AbsInt(iStride) < iRowBytes)
		{
			return BAD_DIMENSIONS;
		}

		void* pScan0 = (void*)(i_pBitmapData->Scan0);

		// Fast path is still available, but only when the bitmap is tightly packed.
		if (iStride == iRowBytes)
		{
			return ilCopyPixels(0, 0, 0, i_iWidth, i_iHeight, 1, IL_BGRA, IL_UNSIGNED_BYTE, pScan0) != NULL
				? OK
				: DevILErrorCode(ilGetError());
		}

		unsigned char* pTemp = (unsigned char*)malloc(iTotalBytes);
		if (pTemp == NULL)
		{
			return OUT_OF_MEMORY;
		}

		DevILErrorCode eResult = OK;
		if (ilCopyPixels(0, 0, 0, i_iWidth, i_iHeight, 1, IL_BGRA, IL_UNSIGNED_BYTE, pTemp) == NULL)
		{
			eResult = DevILErrorCode(ilGetError());
		}
		else
		{
			for (int y = 0; y < i_iHeight; ++y)
			{
				memcpy(BitmapRow(pScan0, iStride, y), pTemp + (y * iRowBytes), iRowBytes);
			}
		}

		free(pTemp);
		return eResult;
	}

	DevILErrorCode UploadBitmapToCurrentDevILImage(
		System::Drawing::Imaging::BitmapData __gc* i_pBitmapData,
		int i_iWidth,
		int i_iHeight)
	{
		int iTotalBytes = 0;
		if (!CheckedPixelBufferBytes(i_iWidth, i_iHeight, 1, &iTotalBytes))
		{
			return BAD_DIMENSIONS;
		}

		const int iRowBytes = i_iWidth * 4;
		const int iStride = i_pBitmapData->get_Stride();

		if (iStride == 0 || AbsInt(iStride) < iRowBytes)
		{
			return BAD_DIMENSIONS;
		}

		void* pScan0 = (void*)(i_pBitmapData->Scan0);

		// Fast path is still available, but only when the bitmap is tightly packed.
		if (iStride == iRowBytes)
		{
			return ilTexImage(i_iWidth, i_iHeight, 1, 4, IL_BGRA, IL_UNSIGNED_BYTE, pScan0) != 0
				? OK
				: DevILErrorCode(ilGetError());
		}

		unsigned char* pTemp = (unsigned char*)malloc(iTotalBytes);
		if (pTemp == NULL)
		{
			return OUT_OF_MEMORY;
		}

		for (int y = 0; y < i_iHeight; ++y)
		{
			memcpy(pTemp + (y * iRowBytes), BitmapRow(pScan0, iStride, y), iRowBytes);
		}

		DevILErrorCode eResult = ilTexImage(i_iWidth, i_iHeight, 1, 4, IL_BGRA, IL_UNSIGNED_BYTE, pTemp) != 0
			? OK
			: DevILErrorCode(ilGetError());

		free(pTemp);
		return eResult;
	}

	DevILErrorCode FillBitmapWhite32(
		System::Drawing::Imaging::BitmapData __gc* i_pBitmapData,
		int i_iWidth,
		int i_iHeight)
	{
		int iTotalBytes = 0;
		if (!CheckedPixelBufferBytes(i_iWidth, i_iHeight, 1, &iTotalBytes))
		{
			return BAD_DIMENSIONS;
		}

		const int iRowBytes = i_iWidth * 4;
		const int iStride = i_pBitmapData->get_Stride();

		if (iStride == 0 || AbsInt(iStride) < iRowBytes)
		{
			return BAD_DIMENSIONS;
		}

		void* pScan0 = (void*)(i_pBitmapData->Scan0);
		for (int y = 0; y < i_iHeight; ++y)
		{
			memset(BitmapRow(pScan0, iStride, y), 0xFF, iRowBytes);
		}

		return OK;
	}
}
