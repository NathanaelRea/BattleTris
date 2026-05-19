#include "BTConfig.H"
#include "BTXmUtils.H"

#include <alloca.h>

XmString
xm_strcat(XmString s1, XmString s2)
{
	XmString str = XmStringConcat(s1, s2);

	if (s1)
    		XmStringFree(s1);

	return (str);
}

XmString
xm_strcreate(const char *buf)
{
	XmString eoln = XmStringSeparatorCreate();
	XmString str = (XmString)NULL, tmp;
	char *p, *s;

	s = (char *) alloca(strlen(buf) + 1);
	(void) strcpy(s, buf);

	for (p = strtok(s, "\n"); p != NULL; p = strtok(NULL, "\n")) {
		if (str == (XmString) NULL) {
			str = XmStringCreateLtoR(p, XmFONTLIST_DEFAULT_TAG);
		} else {
			str = xm_strcat(str, eoln);
			tmp = XmStringCreateLtoR(p, XmFONTLIST_DEFAULT_TAG);
			str = xm_strcat(str, tmp);
			XmStringFree(tmp);
		}
	}

	XmStringFree(eoln);
	return (str);
}
