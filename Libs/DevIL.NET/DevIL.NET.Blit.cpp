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

bool DevIL::DevIL::Blit(System::String __gc* i_szFileNameIn, System::String __gc* i_szFileNameOut,
	int i_iDestX, int i_iDestY, int i_iDestZ,
	int i_iSrcX, int i_iSrcY, int i_iSrcZ,
	int i_iWidth, int i_iHeight, int i_iDepth)
{
	ILuint s_DevilConvertImageNames[2] = { 0, 0 };
	unsigned char* pPixels = NULL;
	bool bRes = false;
	int iTotalBytes = 0;
	s_eErrCode = OK;

	if (IsNullOrEmpty(i_szFileNameIn) || IsNullOrEmpty(i_szFileNameOut))
	{
		s_eErrCode = INVALID_PARAM;
		return false;
	}

	if (!CheckedPixelBufferBytes(i_iWidth, i_iHeight, i_iDepth, &iTotalBytes))
	{
		s_eErrCode = BAD_DIMENSIONS;
		return false;
	}

	EnsureInitialized();

	if (!s_bIluLoaded)
	{
		LoadILU();
	}

	if (!s_bIluLoaded)
	{
		s_eErrCode = ILU_DLL_NOT_FOUND;
		return false;
	}

	pPixels = (unsigned char*)malloc(iTotalBytes);
	if (pPixels == NULL)
	{
		s_eErrCode = OUT_OF_MEMORY;
		return false;
	}

	ilGenImages(2, s_DevilConvertImageNames);

	ilBindImage(s_DevilConvertImageNames[1]);
	if (0 == ilLoadImage(StringAutoMarshal(i_szFileNameIn)))
	{
		s_eErrCode = DevILErrorCode(ilGetError());
		goto cleanup;
	}

	pfnIluImageParameter(ILU_FILTER, ILU_NEAREST);
	if (!pfnIluScale(i_iWidth, i_iHeight, i_iDepth))
	{
		s_eErrCode = DevILErrorCode(ilGetError());
		goto cleanup;
	}

	if (ilCopyPixels(i_iSrcX, i_iSrcY, i_iSrcZ, i_iWidth, i_iHeight, i_iDepth, IL_BGRA, IL_UNSIGNED_BYTE, pPixels) == NULL)
	{
		s_eErrCode = DevILErrorCode(ilGetError());
		goto cleanup;
	}

	ilBindImage(s_DevilConvertImageNames[0]);
	if (0 == ilLoadImage(StringAutoMarshal(i_szFileNameOut)))
	{
		s_eErrCode = DevILErrorCode(ilGetError());
		goto cleanup;
	}

	ilSetPixels(i_iDestX, i_iDestY, i_iDestZ, i_iWidth, i_iHeight, i_iDepth, IL_BGRA, IL_UNSIGNED_BYTE, pPixels);

	ilEnable(IL_FILE_OVERWRITE);
	bRes = ilSaveImage(StringAutoMarshal(i_szFileNameOut)) != 0;
	if (!bRes)
	{
		s_eErrCode = DevILErrorCode(ilGetError());
	}

cleanup:
	if (pPixels != NULL)
	{
		free(pPixels);
		pPixels = NULL;
	}

	if (s_DevilConvertImageNames[0] != 0 || s_DevilConvertImageNames[1] != 0)
	{
		ilDeleteImages(2, s_DevilConvertImageNames);
	}

	return bRes;
}
