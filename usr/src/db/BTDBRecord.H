/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTDBRecord.H                                        */
/*    DATE: Sat Sep  3 18:05:47 1994                            */
/****************************************************************/

#ifndef BTDBRECORD_H
#define BTDBRECORD_H

#include "BTConfig.H"

#include <sys/types.h>

enum {
  BTDBRECORD_KEYLEN = 280
};

class BTDBRecord {

protected:
  char key_[BTDBRECORD_KEYLEN + 1];
  off_t size_;
  int valid_;

public:
  BTDBRecord(const char *key);
  virtual ~BTDBRecord() {}

  operator int() const { return valid_; }
  // operator !() const { return !valid_; }

  virtual short read(int fd, off_t offset, off_t nbytes) = 0;
  virtual short write(int fd, off_t offset) = 0;

  const char *key() const { return (const char *) key_; }
  char *key() { return key_; }
  off_t size() const { return size_; }

  BTDBRecord(const BTDBRecord& other);
  BTDBRecord& operator=(const BTDBRecord& other);

};

#endif
