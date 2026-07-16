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

bool DevIL::DevIL::SaveBitmap(System::String __gc* i_szFileName, System::Drawing::Bitmap __gc* i_poBitmap)
{
	System::Drawing::Bitmap __gc* pWorkBitmap = NULL;
	System::Drawing::Imaging::BitmapData __gc* pBd = NULL;
	ILuint iImageID = 0;
	bool bRes = false;
	s_eErrCode = OK;

	if (IsNullOrEmpty(i_szFileName) || i_poBitmap == NULL)
	{
		s_eErrCode = INVALID_PARAM;
		return false;
	}

	EnsureInitialized();

	int iW = i_poBitmap->get_Width();
	int iH = i_poBitmap->get_Height();

	if (iW <= 0 || iH <= 0)
	{
		s_eErrCode = BAD_DIMENSIONS;
		return false;
	}

	System::Drawing::Rectangle rect;
	rect.X = 0;
	rect.Y = 0;
	rect.Width = iW;
	rect.Height = iH;

	// Do not mutate the caller-owned bitmap. The old code RotateFlip'ed the input in-place
	// and could leave it flipped if an error happened before the final RotateFlip.
	pWorkBitmap = i_poBitmap->Clone(rect, System::Drawing::Imaging::PixelFormat::Format32bppArgb);
	pWorkBitmap->RotateFlip(System::Drawing::RotateFlipType::RotateNoneFlipY);

	ilGenImages(1, &iImageID);
	ilBindImage(iImageID);

	pBd = pWorkBitmap->LockBits(
		rect,
		System::Drawing::Imaging::ImageLockMode::ReadOnly,
		System::Drawing::Imaging::PixelFormat::Format32bppArgb);

	s_eErrCode = UploadBitmapToCurrentDevILImage(pBd, iW, iH);
	if (s_eErrCode != OK)
	{
		goto cleanup;
	}

	bRes = ilSaveImage(StringAutoMarshal(i_szFileName)) != 0;
	if (!bRes)
	{
		s_eErrCode = DevILErrorCode(ilGetError());
	}

cleanup:
	if (pBd != NULL)
	{
		pWorkBitmap->UnlockBits(pBd);
		pBd = NULL;
	}

	if (iImageID != 0)
	{
		ilDeleteImages(1, &iImageID);
	}

	if (pWorkBitmap != NULL)
	{
		pWorkBitmap->Dispose();
		pWorkBitmap = NULL;
	}

	return bRes;
}

bool DevIL::DevIL::NewBitMap(System::String __gc* i_szFileNameIn, int i_iWidth, int i_iHeight)
{
	System::Drawing::Bitmap __gc* pBmp = NULL;
	System::Drawing::Imaging::BitmapData __gc* pBd = NULL;
	bool bRes = false;
	s_eErrCode = OK;

	if (IsNullOrEmpty(i_szFileNameIn) || i_iWidth <= 0 || i_iHeight <= 0)
	{
		s_eErrCode = INVALID_PARAM;
		return false;
	}

	pBmp = __gc new System::Drawing::Bitmap(i_iWidth, i_iHeight, System::Drawing::Imaging::PixelFormat::Format32bppArgb);

	System::Drawing::Rectangle rect;
	rect.X = 0;
	rect.Y = 0;
	rect.Width = i_iWidth;
	rect.Height = i_iHeight;

	pBd = pBmp->LockBits(
		rect,
		System::Drawing::Imaging::ImageLockMode::WriteOnly,
		System::Drawing::Imaging::PixelFormat::Format32bppArgb);

	s_eErrCode = FillBitmapWhite32(pBd, i_iWidth, i_iHeight);

	if (pBd != NULL)
	{
		pBmp->UnlockBits(pBd);
		pBd = NULL;
	}

	if (s_eErrCode == OK)
	{
		ilEnable(IL_FILE_OVERWRITE);
		bRes = SaveBitmap(i_szFileNameIn, pBmp);
	}

	if (pBmp != NULL)
	{
		pBmp->Dispose();
		pBmp = NULL;
	}

	return bRes;
}
