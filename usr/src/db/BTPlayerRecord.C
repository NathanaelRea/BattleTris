/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTPlayerRecord.C                                    */
/*    DATE: Thu May  5 01:46:53 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <assert.h>
#include <string.h>
#include <strings.h>

#include "BTPlayerRecord.H"

BTPlayerRecord::BTPlayerRecord(const BTPlayerRecord& other)
{
  bzero(opponent_, sizeof(opponent_));
  strncpy(opponent_, other.opponent_, BTDBRECORD_KEYLEN);
  wins_ = other.wins_;
  losses_ = other.losses_;
}

BTPlayerRecord& BTPlayerRecord::operator=(const BTPlayerRecord& other)
{
  if(this == &other)
    return *this;

  strncpy(opponent_, other.opponent_, BTDBRECORD_KEYLEN);
  wins_ = other.wins_;
  losses_ = other.losses_;

  return *this;
}

int BTPlayerRecord::operator==(const BTPlayerRecord& other)
{
  return strcmp(opponent_, other.opponent_) == 0;
}

int BTPlayerRecord::operator!=(const BTPlayerRecord& other)
{
  return strcmp(opponent_, other.opponent_) != 0;
}

int BTPlayerRecord::compare(const void *left, const void *right)
{
  assert(left != 0);
  assert(right != 0);

  BTPlayerRecord *lval = *((BTPlayerRecord **) left);
  BTPlayerRecord *rval = *((BTPlayerRecord **) right);

  assert(lval != 0);
  assert(rval != 0);

  return strcmp(lval->opponent_, rval->opponent_);
}

#ifndef NDEBUG
ostream& operator<<(ostream& os, BTPlayerRecord& record)
{
  os << "Record against " << record.opponent_ << ":\n";
  return os << record.wins_ << " wins, " << record.losses_ << " losses\n";
}
#endif
