/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: StreamSocketErr.C                                   */
/*    DATE: Wed Jan 26 23:50:16 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <string.h>

#include "BTDBErr.H"

static char errmsg[255];

char *BTDBErrMsg(short errcode)
{
  switch(errcode) {
  case ERRBTDBNOERR:
    strncpy(errmsg, "No error occurred", sizeof(errmsg));
    break;

  case ERRBTDBCREATE:
    strncpy(errmsg, "Failed to create file", sizeof(errmsg));
    break;

  case ERRBTDBOPEN:
    strncpy(errmsg, "Failed to open file", sizeof(errmsg));
    break;

  case ERRBTDBREMOVE:
    strncpy(errmsg, "Failed to unlink file", sizeof(errmsg));
    break;

  case ERRBTDBTIMEOUT:
    strncpy(errmsg, "Timed out waiting to lock file", sizeof(errmsg));
    break;

  case ERRBTDBCLOSE:
    strncpy(errmsg, "Failed to close file", sizeof(errmsg));
    break;

  case ERRBTDBREAD:
    strncpy(errmsg, "Failed to read from file", sizeof(errmsg));
    break;

  case ERRBTDBWRITE:
    strncpy(errmsg, "Failed to write to file", sizeof(errmsg));
    break;

  case ERRBTDBSTAT:
    strncpy(errmsg, "Failed to stat file", sizeof(errmsg));	
    break;

  case ERRBTDBLOCK:
    strncpy(errmsg, "Failed to lock byte range", sizeof(errmsg));
    break;

  case ERRBTDBSEEK:
    strncpy(errmsg, "Failed to seek file", sizeof(errmsg));
    break;

  case ERRBTDBCORRUPT:
    strncpy(errmsg, "Database file appears to be corrupt", sizeof(errmsg));
    break;

  default:
    strncpy(errmsg, "Unknown database error", sizeof(errmsg));
  }

  return errmsg;
}
