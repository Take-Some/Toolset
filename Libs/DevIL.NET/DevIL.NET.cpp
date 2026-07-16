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
	ILuint DevIL::s_iImageID = 0;
	bool DevIL::s_bInitDone = false;
	bool DevIL::s_bIluLoaded = false;
	bool DevIL::s_bIluLoadAttempted = false;
	HMODULE DevIL::s_hIluModule = NULL;
	DevILErrorCode DevIL::s_eErrCode = OK;
	PFNILUSCALE DevIL::pfnIluScale = NULL;
	PFNILUIMAGEPARAMETER DevIL::pfnIluImageParameter = NULL;
}

void DevIL::DevIL::EnsureInitialized()
{
	if (!s_bInitDone)
	{
		ilInit();
		s_bInitDone = true;
	}
}

void DevIL::DevIL::LoadILU()
{
	if (s_bIluLoadAttempted)
	{
		return;
	}

	s_bIluLoadAttempted = true;
	s_bIluLoaded = false;
	pfnIluScale = NULL;
	pfnIluImageParameter = NULL;

	s_hIluModule = LoadLibraryA("ILU.dll");
	if (s_hIluModule != NULL)
	{
		pfnIluScale = (PFNILUSCALE)GetProcAddress(s_hIluModule, "iluScale");
		pfnIluImageParameter = (PFNILUIMAGEPARAMETER)GetProcAddress(s_hIluModule, "iluImageParameter");
		s_bIluLoaded = (pfnIluScale != NULL) && (pfnIluImageParameter != NULL);

		if (!s_bIluLoaded)
		{
			FreeLibrary(s_hIluModule);
			s_hIluModule = NULL;
			pfnIluScale = NULL;
			pfnIluImageParameter = NULL;
		}
	}
}

System::Drawing::Bitmap __gc* DevIL::DevIL::LoadBitmap(System::String __gc* i_szFileName)
{
	return LoadBitmapAndScale(i_szFileName, 0, 0, NEAREST, DO_NOT_SCALE);
}

DevIL::DevILErrorCode DevIL::DevIL::GetErrorCode()
{
	if (s_eErrCode == OK)
	{
		return DevILErrorCode(ilGetError());
	}
	else
	{
		DevILErrorCode eErr = s_eErrCode;
		s_eErrCode = OK;
		return eErr;
	}
}
