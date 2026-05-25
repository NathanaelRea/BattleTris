/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTGameStats.C                                       */
/*    DATE: Thu May  5 03:45:56 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <string.h>
#include <strings.h>

#include "BTGameStats.H"
#include "BTNetwork.H"

BTGameStats::BTGameStats()
: winnerScore_(0), winnerLines_(0), winnerFunds_(0),
  loserScore_(0), loserLines_(0), loserFunds_(0), duration_(0)
{
  bzero((char *) winnerName_, sizeof(winnerName_));
  bzero((char *) loserName_, sizeof(loserName_));
}

char *BTGameStats::writebuf(char *bufptr)
{
  unsigned long tl;

  bcopy((char *) winnerName_, (char *) bufptr, sizeof(winnerName_));
  bufptr += sizeof(winnerName_);

  BTNET_PUTLONG(bufptr, tl, winnerScore_);
  BTNET_PUTLONG(bufptr, tl, winnerLines_);
  BTNET_PUTLONG(bufptr, tl, winnerFunds_);

  bcopy((char *) loserName_, (char *) bufptr, sizeof(loserName_));
  bufptr += sizeof(loserName_);

  BTNET_PUTLONG(bufptr, tl, loserScore_);
  BTNET_PUTLONG(bufptr, tl, loserLines_);
  BTNET_PUTLONG(bufptr, tl, loserFunds_);

  BTNET_PUTLONG(bufptr, tl, duration_);

  return bufptr;
}

char *BTGameStats::readbuf(char *bufptr)
{
  unsigned long tl;

  bcopy((char *) bufptr, (char *) winnerName_, sizeof(winnerName_));
  bufptr += sizeof(winnerName_);

  BTNET_GETLONG(bufptr, tl, winnerScore_);
  BTNET_GETLONG(bufptr, tl, winnerLines_);
  BTNET_GETLONG(bufptr, tl, winnerFunds_);

  bcopy((char *) bufptr, (char *) loserName_, sizeof(loserName_));
  bufptr += sizeof(loserName_);

  BTNET_GETLONG(bufptr, tl, loserScore_);
  BTNET_GETLONG(bufptr, tl, loserLines_);
  BTNET_GETLONG(bufptr, tl, loserFunds_);

  BTNET_GETLONG(bufptr, tl, duration_);

  return bufptr;
}

BTGameStats::BTGameStats(const BTGameStats& other)
: winnerScore_(other.winnerScore_), winnerLines_(other.winnerLines_),
  winnerFunds_(other.winnerFunds_), loserScore_(other.loserScore_),
  loserLines_(other.loserLines_), loserFunds_(other.loserFunds_),
  duration_(other.duration_)
{
  strncpy(winnerName_, other.winnerName_, BTDBRECORD_KEYLEN);
  strncpy(loserName_, other.loserName_, BTDBRECORD_KEYLEN);
}

BTGameStats& BTGameStats::operator=(const BTGameStats& other)
{
  if(this == &other)
    return *this;

  winnerScore_ = other.winnerScore_;
  winnerLines_ = other.winnerLines_;
  winnerFunds_ = other.winnerFunds_;

  loserScore_ = other.loserScore_;
  loserLines_ = other.loserLines_;
  loserFunds_ = other.loserFunds_;

  strncpy(winnerName_, other.winnerName_, BTDBRECORD_KEYLEN);
  strncpy(loserName_, other.loserName_, BTDBRECORD_KEYLEN);

  duration_ = other.duration_;

  return *this;
}
