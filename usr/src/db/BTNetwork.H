/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTNetwork.H                                         */
/*    DATE: Fri Dec 23 02:27:16 1994                            */
/****************************************************************/

#ifndef BTNETWORK_H
#define BTNETWORK_H

#include "BTConfig.H"

#include <netinet/in.h>
#include <strings.h>

#define BTNET_PUTLONG(bufptr, tmp, x) \
tmp = htonl(x); \
bcopy((char *) &tmp, (char *) bufptr, sizeof(unsigned long)); \
bufptr += sizeof(unsigned long)

#define BTNET_PUTSHORT(bufptr, tmp, x) \
tmp = htons(x); \
bcopy((char *) &tmp, (char *) bufptr, sizeof(unsigned short)); \
bufptr += sizeof(unsigned short)

#define BTNET_GETLONG(bufptr, tmp, x) \
bcopy((char *) bufptr, (char *) &tmp, sizeof(unsigned long)); \
bufptr += sizeof(unsigned long); \
x = ntohl(tmp)

#define BTNET_GETSHORT(bufptr, tmp, x) \
bcopy((char *) bufptr, (char *) &tmp, sizeof(unsigned short)); \
bufptr += sizeof(unsigned short); \
x = ntohs(tmp)

#endif
