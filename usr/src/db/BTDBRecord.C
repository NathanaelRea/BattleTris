/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTDBRecord.C                                        */
/*    DATE: Sat Sep  3 22:30:46 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <assert.h>
#include <string.h>
#include <strings.h>

#include "BTDBRecord.H"

BTDBRecord::BTDBRecord(const char *key)
: valid_(0), size_(sizeof(key_))
{
  assert(key != 0);

  bzero((char *) key_, sizeof(key_));
  strncpy(key_, key, BTDBRECORD_KEYLEN);
}

BTDBRecord::BTDBRecord(const BTDBRecord& other)
{
  bzero((char *) key_, sizeof(key_));
  strncpy(key_, other.key_, BTDBRECORD_KEYLEN);
  size_ = other.size_;
  valid_ = other.valid_;
}

BTDBRecord& BTDBRecord::operator=(const BTDBRecord& other)
{
  if(&other == this)
    return *this;

  strncpy(key_, other.key_, BTDBRECORD_KEYLEN);
  size_ = other.size_;
  valid_ = other.valid_;

  return *this;
}
