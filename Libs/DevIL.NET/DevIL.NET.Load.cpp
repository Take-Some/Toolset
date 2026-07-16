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

System::Drawing::Bitmap __gc* DevIL::DevIL::LoadBitmapAndScale(System::String __gc* i_szFileName,
	int i_iWidth, int i_iHeight,
	DevILScaleFilter i_eFilter,
	DevILScaleKind i_eKind)
{
	System::Drawing::Bitmap __gc* pBmp = NULL;
	System::Drawing::Imaging::BitmapData __gc* pBd = NULL;
	ILuint iImageID = 0;
	s_eErrCode = OK;

	if (IsNullOrEmpty(i_szFileName))
	{
		s_eErrCode = INVALID_PARAM;
		return NULL;
	}

	EnsureInitialized();

	ilGenImages(1, &iImageID);
	ilBindImage(iImageID);

	if (0 == ilLoadImage(StringAutoMarshal(i_szFileName)))
	{
		s_eErrCode = DevILErrorCode(ilGetError());
		goto cleanup;
	}

	int iSourceW = ilGetInteger(IL_IMAGE_WIDTH);
	int iSourceH = ilGetInteger(IL_IMAGE_HEIGHT);
	int iW = iSourceW;
	int iH = iSourceH;
	bool bResize = true;

	switch(i_eKind)
	{
		case WIDTH_AND_HEIGHT:
			iW = i_iWidth;
			iH = i_iHeight;
			break;
		case WIDTH_ONLY:
			iW = i_iWidth;
			break;
		case HEIGHT_ONLY:
			iH = i_iHeight;
			break;
		case KEEPRATIO_USING_WIDTH:
			if (iSourceW <= 0)
			{
				s_eErrCode = BAD_DIMENSIONS;
				goto cleanup;
			}
			iH = int(double(iSourceH) * double(i_iWidth) / double(iSourceW));
			iW = i_iWidth;
			break;
		case KEEPRATIO_USING_HEIGHT:
			if (iSourceH <= 0)
			{
				s_eErrCode = BAD_DIMENSIONS;
				goto cleanup;
			}
			iW = int(double(iSourceW) * double(i_iHeight) / double(iSourceH));
			iH = i_iHeight;
			break;
		case DO_NOT_SCALE:
			bResize = false;
			break;
		default:
			s_eErrCode = INVALID_ENUM;
			goto cleanup;
	}

	if (iW <= 0 || iH <= 0)
	{
		s_eErrCode = BAD_DIMENSIONS;
		goto cleanup;
	}

	if (bResize)
	{
		if (!s_bIluLoaded)
		{
			LoadILU();
		}

		if (s_bIluLoaded)
		{
			pfnIluImageParameter(ILU_FILTER, (int)(i_eFilter));
			if (!pfnIluScale(iW, iH, 1))
			{
				s_eErrCode = DevILErrorCode(ilGetError());
				goto cleanup;
			}
		}
		else
		{
			s_eErrCode = ILU_DLL_NOT_FOUND;
			goto cleanup;
		}
	}

	if (0 == ilConvertImage(IL_BGRA, IL_UNSIGNED_BYTE))
	{
		s_eErrCode = DevILErrorCode(ilGetError());
		goto cleanup;
	}

	pBmp = __gc new System::Drawing::Bitmap(iW, iH, System::Drawing::Imaging::PixelFormat::Format32bppArgb);

	System::Drawing::Rectangle rect;
	rect.X = 0;
	rect.Y = 0;
	rect.Width = iW;
	rect.Height = iH;

	pBd = pBmp->LockBits(
		rect,
		System::Drawing::Imaging::ImageLockMode::WriteOnly,
		System::Drawing::Imaging::PixelFormat::Format32bppArgb);

	s_eErrCode = CopyCurrentDevILImageToBitmap(pBd, iW, iH);

cleanup:
	if (pBd != NULL)
	{
		pBmp->UnlockBits(pBd);
		pBd = NULL;
	}

	if (iImageID != 0)
	{
		ilDeleteImages(1, &iImageID);
	}

	if (s_eErrCode != OK)
	{
		return NULL;
	}

	return pBmp;
}
