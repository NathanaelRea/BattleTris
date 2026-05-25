/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTDB.C                                              */
/*    DATE: Tue Sep  6 21:21:06 1994                            */
/****************************************************************/

#include "BTConfig.H"

#ifndef NDEBUG
# include <iostream.h>
#endif 

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#include <assert.h>
#include <fcntl.h>
#include <string.h>
#include <strings.h>

#include "BTDB.H"
#include "BTDBRecord.H"
#include "BTDBReadLock.H"
#include "BTDBWriteLock.H"

const int BTDB_NHASH_DEFAULT = 137;			// hash table size
const size_t BTDB_PTR_SIZE = sizeof(off_t);		// size of ptr field
const off_t BTDB_FREELIST_OFFSET = 0;			// freelist offset
const off_t BTDB_HASHLIST_OFFSET = BTDB_PTR_SIZE;	// hashlist offset

const mode_t BTDB_MODE_DEFAULT = S_IRUSR | S_IWUSR | S_IRGRP | S_IWGRP;

ssize_t writen(int fd, const void *buf, size_t nbytes)
{
  size_t nwritten, nleft = nbytes;
  const char *bufptr = (const char *) buf;

  while(nleft > 0) {
    if((nwritten = write(fd, bufptr, nleft)) <= 0) {
      if(errno == EINTR)
        continue;
      return nwritten;
    }

    nleft -= nwritten;
    bufptr += nwritten;
  }

  return nbytes;
}

BTDB::BTDB(const char *pathname, int oflags)
: errcode_(ERRBTDBNOERR), valid_(1), idxfd_(-1), datfd_(-1),
  oflags_(oflags), idxoff_(0), idxlen_(0), datoff_(0), datlen_(0),
  ptrval_(0), ptroff_(0), chnoff_(0), hshoff_(BTDB_HASHLIST_OFFSET),
  nhash_(BTDB_NHASH_DEFAULT), findlock_(0)
{
  assert(pathname != 0);
  size_t len = strlen(pathname);

  dbname_ = new char [len + 5]; // 5 = 4 chars from suffix plus null byte

  idxbuflen_ = BTDBRECORD_KEYLEN + 1 + sizeof(off_t) + sizeof(off_t);
  idxbuf_ = new char [idxbuflen_];

  strcpy(dbname_, pathname);
  strcat(dbname_, BTDB_IDXSUFFIX);

  if((idxfd_ = ::open(dbname_, oflags_, BTDB_MODE_DEFAULT)) < 0) {
    delete [] dbname_;
    delete [] idxbuf_;
    errcode_ = ERRBTDBOPEN;
    valid_ = 0;
    return;
  }

  strcpy((char *) (dbname_ + len), BTDB_DATSUFFIX);

  if((datfd_ = ::open(dbname_, oflags_, BTDB_MODE_DEFAULT)) < 0) {
    delete [] dbname_;
    delete [] idxbuf_;
    errcode_ = ERRBTDBOPEN;
    valid_ = 0;
    return;
  }

  if((oflags_ & O_CREAT) || (oflags_ & O_TRUNC)) {

    // Take a write-lock on the entire file so we can stat the file,
    // check its size, and initialize it as an atomic operation

    BTDBWriteLock idxlock(idxfd_);
    idxlock.lockw();

    struct stat statbuf;

    if(fstat(idxfd_, &statbuf) < 0) {
      errcode_ = ERRBTDBSTAT;
      valid_ = 0;
      return;
    }

    if(statbuf.st_size == 0) {

      // Since the file is empty, we need to build a list of
      // BTDB_NHASH_DEFAULT chain pointers set to zero, with an extra one
      // at the beginning for the free list pointer preceding the hash table

      char hash[(BTDB_NHASH_DEFAULT + 1) * sizeof(off_t)];
      bzero((char *) hash, sizeof(hash));

      if(writen(idxfd_, (void *) hash, sizeof(hash)) != sizeof(hash)) {
	errcode_ = ERRBTDBWRITE;
	valid_ = 0;
	return;
      }
    }

    idxlock.unlock();
  }

  BTDB::rewind();
}

BTDB::~BTDB()
{
  if(findlock_ != 0)
    delete findlock_;

  if(dbname_)
    delete [] dbname_;
  
  if(idxbuf_)
    delete [] idxbuf_;

  close(idxfd_);
  close(datfd_);
}

int BTDB::fetch(BTDBRecord *record)
{
  assert(record != 0);

  if(!valid_)
    return 0;

  errcode_ = ERRBTDBNOERR;
  int found = 0;

  if(dbfind(record, 0))
    if((errcode_ = record->read(datfd_, datoff_, datlen_)) == ERRBTDBNOERR)
      found = 1;

  if(findlock_ != 0)
    findlock_->unlock();

  return found;
}

int BTDB::remove(BTDBRecord *record)
{
  assert(record != 0);

  if(!valid_)
    return 0;

  errcode_ = ERRBTDBNOERR;
  int found = 0;

  if(dbfind(record, 1)) {
    dbdelete(record);
    if(errcode_ == ERRBTDBNOERR)
      found = 1;
  }

  if(findlock_ != 0)
    findlock_->unlock();

  return found;
}

int BTDB::insert(BTDBRecord *record)
{
  assert(record != 0);

  off_t ptrval;
  int found;

  if(!valid_)
    return 0;

  errcode_ = ERRBTDBNOERR;

  // If the record already exists, then insert fails

  if(dbfind(record, 1))
    goto dberr;

  // dbfind already locked the hash-chain for us, so just go ahead and read
  // the chain pointer to the first index record on the hash chain

  ptrval = dbreadptr(chnoff_);
  if(errcode_ != ERRBTDBNOERR)
    goto dberr;

  found = dbfindfree(idxbuflen_, record->size());
  if(errcode_ != ERRBTDBNOERR)
    goto dberr;

  if(found) {

    // We can reuse an empty record.  dbfindfree already removed the record
    // from the freelist and set both datoff_ and idxoff_

#ifndef NDEBUG
    cout << "DEBUG: dbfindfree successful" << endl;
    cout << "DEBUG: dbwriterec to offset " << datoff_ << endl;
    cout << "DEBUG: dbwriteidx to offset " << idxoff_ << endl;
#endif

    dbwriterec(record, datoff_, SEEK_SET);
    if(errcode_ != ERRBTDBNOERR)
      goto dberr;

    dbwriteidx(record->key(), idxoff_, SEEK_SET, ptrval);
    if(errcode_ != ERRBTDBNOERR)
      goto dberr;

    // The reused record goes to the front of the hash chain

    dbwriteptr(chnoff_, idxoff_);
    if(errcode_ != ERRBTDBNOERR)
      goto dberr;

  } else {

    // An empty record of the correct size was not found, so we need to
    // append the new records to the end of the index and data files

#ifndef NDEBUG
    cout << "DEBUG: dbfindfree failed" << endl;
    cout << "DEBUG: dbwriterec to end of file" << endl;
    cout << "DEBUG: dbwriteidx to end of file" << endl;
#endif

    dbwriterec(record, 0, SEEK_END);
    if(errcode_ != ERRBTDBNOERR)
      goto dberr;

    dbwriteidx(record->key(), 0, SEEK_END, ptrval);
    if(errcode_ != ERRBTDBNOERR)
      goto dberr;

    // The new record goes to the front of the hash chain

    dbwriteptr(chnoff_, idxoff_);
    if(errcode_ != ERRBTDBNOERR)
      goto dberr;
  }

  if(findlock_ != 0)
    findlock_->unlock();

  return 1;

 dberr:
  if(findlock_ != 0)
    findlock_->unlock();
  return 0;
}

int BTDB::replace(BTDBRecord *record)
{
  assert(record != 0);

  if(!valid_)
    return 0;

  errcode_ = ERRBTDBNOERR;

  // If the record does not already exist, then replace fails

  if(!dbfind(record, 1))
    goto dberr;

  if(errcode_ != ERRBTDBNOERR)
    goto dberr;

  // We are replacing a record already in the database, so we know that
  // the new key is the same as the key already stored in the index record,
  // but we need to check if the data records are the same size

  if(record->size() != datlen_) {

    // Delete the existing record

    dbdelete(record);
    if(errcode_ != ERRBTDBNOERR)
      goto dberr;

    // Reread the chain pointer from the hash table since it may have
    // changed during dbdelete

    off_t ptrval = dbreadptr(chnoff_);
    if(errcode_ != ERRBTDBNOERR)
      goto dberr;

    // Append the new records to the end of the index and data files

    dbwriterec(record, 0, SEEK_END);
    if(errcode_ != ERRBTDBNOERR)
      goto dberr;

    dbwriteidx(record->key(), 0, SEEK_END, ptrval);
    if(errcode_ != ERRBTDBNOERR)
      goto dberr;

    // The new record goes to the front of the hash chain

    dbwriteptr(chnoff_, idxoff_);
    if(errcode_ != ERRBTDBNOERR)
      goto dberr;

  } else {

    // The record size is the same, so just write out the new data

    dbwriterec(record, datoff_, SEEK_SET);
    if(errcode_ != ERRBTDBNOERR)
      goto dberr;
  }

  if(findlock_ != 0)
    findlock_->unlock();

  return 1;

 dberr:
  if(findlock_ != 0)
    findlock_->unlock();
  return 0;
}

void BTDB::rewind()
{
  if(!valid_)
    return;

  errcode_ = ERRBTDBNOERR;

  // We want to seek past the hash table (nhash_ pointers) and +1 for
  // the freelist pointer

  off_t offset = (nhash_ + 1) * BTDB_PTR_SIZE;

  if((idxoff_ = lseek(idxfd_, offset, SEEK_SET)) < 0)
    errcode_ = ERRBTDBSEEK;
}

int BTDB::nextrec(BTDBRecord *record)
{
  assert(record != 0);

  if(!valid_)
    return 0;

  errcode_ = ERRBTDBNOERR;

  // We need to read-lock the freelist so that we don\'t read a record
  // in the middle of its being deleted

  BTDBReadLock idxlock(BTDB_FREELIST_OFFSET, 1);
  idxlock.lockw();

  char c;

  do {

    // Read the next sequential index record, and return 0 if EOF reached

    if(dbreadidx(0) < 0) {
      idxlock.unlock();
      return 0;
    }

    // Check to see if the key is all blank (empty record)
    
    for(char *ptr = idxbuf_; ((c = *ptr++) != 0) && (c == ' '););

  } while(c == 0);

  errcode_ = record->read(datfd_, datoff_, datlen_);
  idxlock.unlock();

  if(errcode_ != ERRBTDBNOERR)
    return 0;

  return 1;
}

int BTDB::dbfind(BTDBRecord *record, int writelock)
{
  assert(record != 0);

  off_t offset, nextoffset;

  // Calculate the hash value for the key, and then calculate the byte offset
  // of the corresponding chain pointer in the hash table.  Then begin
  // searching for the key at this location in the index file.

  chnoff_ = (dbhash(record->key()) * BTDB_PTR_SIZE) + hshoff_;
  ptroff_ = chnoff_;

  // Lock the first byte of the hash chain.  IMPORTANT: It is the caller\'s
  // responsibility to release this lock!

  if(findlock_ != 0) {
    delete findlock_;
    findlock_ = 0;
  }

  if(writelock)
    findlock_ = new BTDBWriteLock(idxfd_, chnoff_, 1);
  else
    findlock_ = new BTDBReadLock(idxfd_, chnoff_, 1);

  findlock_->lockw();

  offset = dbreadptr(ptroff_);

  if(errcode_ != ERRBTDBNOERR)
    return 0;

  // Linearly search the keys on this hash chain until we find what we\'re
  // looking for, or we reach a null chain pointer

  while(offset != 0) {
    nextoffset = dbreadidx(offset);

    if(errcode_ != ERRBTDBNOERR)
      return 0;

    if(nextoffset == offset) {

#ifndef NDEBUG
      cerr << "DEBUG: Index record at " << offset << " is corrupt!" << endl;
#endif

      errcode_ = ERRBTDBCORRUPT;
    }

    if(strncmp(idxbuf_, record->key(), BTDBRECORD_KEYLEN) == 0)
      break;

    ptroff_ = offset;
    offset = nextoffset;
  }

  if(offset == 0)
    return 0;

  return 1;
}

btdb_hash_t BTDB::dbhash(const char *key)
{
  assert(key != 0);

  btdb_hash_t hval = 0;
  const char *ptr;
  char c;
  int i;

  for(ptr = key, i = 1; (c = *ptr++); i++)
    hval += c * i;

  return hval % nhash_;
}

off_t BTDB::dbreadptr(off_t offset)
{
  off_t ptrval;

  // Seek to the specified offset, and then read an off_t value

  if(lseek(idxfd_, offset, SEEK_SET) < 0) {
    errcode_ = ERRBTDBSEEK;
    return -1;
  }

  if(::read(idxfd_, (void *) &ptrval, sizeof(ptrval)) != sizeof(ptrval)) {
    errcode_ = ERRBTDBREAD;
    return -1;
  }

#ifndef NDEBUG
  cout << "DEBUG: Read ptr " << ptrval << " from offset " << offset << endl;
#endif

  return ptrval;
}

off_t BTDB::dbreadidx(off_t offset)
{
  char idxhdr[sizeof(off_t) + sizeof(off_t)];
  ssize_t nbytes;

  // dbnextrec calls this method with offset == 0, which indicates seek
  // from the current location, rather than to an absolute location

  if((idxoff_ = lseek(idxfd_, offset,
			offset == 0 ? SEEK_CUR : SEEK_SET)) < 0) {
    errcode_ = ERRBTDBSEEK;
    return -1;
  }
  
  if((nbytes = ::read(idxfd_, (void *) idxhdr, sizeof(idxhdr)))
     != sizeof(idxhdr)) {

    // Check for EOF indicator for BTDB::nextrec

    if((offset == 0) && (nbytes == 0))
      return -1;

    errcode_ = ERRBTDBREAD;
    return -1;
  }

  char *bufptr = idxhdr;

  // Copy the chain pointer and the index record size from the index
  // record header

  bcopy((char *) bufptr, (char *) &ptrval_, sizeof(ptrval_));
  bufptr += sizeof(ptrval_);
  bcopy((char *) bufptr, (char *) &idxlen_, sizeof(idxlen_));

  // If the index record is somehow bigger than our buffer, then
  // assume the record is corrupted

  if(idxlen_ > idxbuflen_) {
    errcode_ = ERRBTDBCORRUPT;
    return -1;
  }

  // Now read the rest of the index record into the index record buffer

  if(::read(idxfd_, (void *) idxbuf_, idxlen_) != idxlen_) {
    errcode_ = ERRBTDBREAD;
    return -1;
  }

  // Skip past the index record key and terminating null-byte

  bufptr = idxbuf_ + BTDBRECORD_KEYLEN + 1;

  // Copy the data offset and data size from the index buffer

  bcopy((char *) bufptr, (char *) &datoff_, sizeof(datoff_));
  bufptr += sizeof(datoff_);
  bcopy((char *) bufptr, (char *) &datlen_, sizeof(datlen_));

  return ptrval_;
}

void BTDB::dbdelete(BTDBRecord *record)
{
  assert(record != 0);

  // Fill the entire index buffer with blanks

  for(char *ptr = idxbuf_; *ptr != '\0'; ptr++)
    *ptr = ' ';

  // Obtain a write-lock on the freelist

  BTDBWriteLock freelock(idxfd_, BTDB_FREELIST_OFFSET, 1);
  freelock.lockw();

  // Read the freelist pointer.  Its value now becomes the chain pointer
  // field of the deletex index record.  The deleted record becomes the
  // head of the freelist.

  off_t freeptr = dbreadptr(BTDB_FREELIST_OFFSET);

  if(errcode_ != ERRBTDBNOERR)
    return;

  off_t saveptr = ptrval_;

  // Rewrite the entire index record

#ifndef NDEBUG
  cout << "DEBUG: Rewriting empty index record at offset " << idxoff_ << endl;
  cout << "DEBUG: offset = " << idxoff_ << "; chain ptr = " << freeptr << endl;
#endif

  if(idxoff_ == freeptr) {

#ifndef NDEBUG
    cerr << "DEBUG: About to write corrupt idx record at " << idxoff_ << endl;
#endif

    errcode_ = ERRBTDBCORRUPT;
    return;
  }

  dbwriteidx(idxbuf_, idxoff_, SEEK_SET, freeptr);

  if(errcode_ != ERRBTDBNOERR)
    return;

  // Write the new freelist pointer

#ifndef NDEBUG
  cout << "DEBUG: Changing freelist pointer to " << idxoff_ << endl;
#endif

  dbwriteptr(BTDB_FREELIST_OFFSET, idxoff_);

  if(errcode_ != ERRBTDBNOERR)
    return;

  // Rewrite the chain pointer which pointed to the record being deleted.
  // dbfind sets ptroff_ to point to this chain pointer.  Set this chain
  // pointer to point the contents of the deleted record\'s chain pointer.

  dbwriteptr(ptroff_, saveptr);

  if(errcode_ != ERRBTDBNOERR)
    return;

  // Release the lock on the freelist

  freelock.unlock();
}

void BTDB::dbwriterec(BTDBRecord *record, off_t offset, int whence)
{
  assert(record != 0);

  BTDBWriteLock datalock(datfd_);

  // If we\'re appending a new record, we have to take out a write-lock on
  // the entire file in order to make lseek() and write() an atomic
  // operation.  This is not needed if we\'re overwriting an existing
  // record.

  if(whence == SEEK_END)
    datalock.lockw();

  if((datoff_ = lseek(datfd_, offset, whence)) < 0) {
    errcode_ = ERRBTDBSEEK;
    return;
  }

  datlen_ = record->size();
  record->write(datfd_, datoff_);

  if(whence == SEEK_END)
    datalock.unlock();
}

void BTDB::dbwriteidx(const char *key, off_t offset, int whence, off_t ptr)
{
  assert(key != 0);
  assert(ptr >= 0);

  // Declare a temporary buffer for the index record header

  char idxhdr[sizeof(off_t) + sizeof(off_t)];
  char *bufptr = idxhdr;

  // Copy the key into the index record buffer

  if(key != idxbuf_)
    strncpy(idxbuf_, key, BTDBRECORD_KEYLEN);

  // Fill in the index record header

  bcopy((char *) &ptr, (char *) bufptr, sizeof(ptr));
  bufptr += sizeof(ptr);
  bcopy((char *) &idxbuflen_, (char *) bufptr, sizeof(idxbuflen_));

  // Fill in the data offset and data size in the index record buffer

  bufptr = idxbuf_ + BTDBRECORD_KEYLEN + 1;

  bcopy((char *) &datoff_, (char *) bufptr, sizeof(datoff_));
  bufptr += sizeof(datoff_);
  bcopy((char *) &datlen_, (char *) bufptr, sizeof(datlen_));

  // If we\'re appending to the index file, we need to write-lock before
  // doing the lseek and write operations to make these atomic.  If we\'re
  // overwriting an existing record, then we don\'t have to worry about this.

  BTDBWriteLock idxlock(idxfd_, ((nhash_ + 1) * BTDB_PTR_SIZE) + 1, 0);

  if(whence == SEEK_END)
    idxlock.lockw();

  // Seek to the specified offset and write the index header and buffer

  if((idxoff_ = lseek(idxfd_, offset, whence)) < 0) {
    errcode_ = ERRBTDBSEEK;
    idxlock.unlock();
    return;
  }

  if(writen(idxfd_, (void *) idxhdr, sizeof(idxhdr)) != sizeof(idxhdr)) {
    errcode_ = ERRBTDBWRITE;
    idxlock.unlock();
    return;
  }

  if(writen(idxfd_, (void *) idxbuf_, idxbuflen_) != idxbuflen_) {
    errcode_ = ERRBTDBWRITE;
    idxlock.unlock();
    return;
  }

  idxlock.unlock();
}

void BTDB::dbwriteptr(off_t offset, off_t ptr)
{
  assert(ptr >= 0);

  if(lseek(idxfd_, offset, SEEK_SET) < 0) {
    errcode_ = ERRBTDBSEEK;
    return;
  }

  if(writen(idxfd_, (void *) &ptr, sizeof(ptr)) != sizeof(ptr)) {
    errcode_ = ERRBTDBWRITE;
    return;
  }

#ifndef NDEBUG
  cout << "DEBUG: Wrote ptr " << ptr << " to offset " << offset << endl;
#endif

}

int BTDB::dbfindfree(off_t idxlen, off_t datlen)
{
  BTDBWriteLock idxlock(idxfd_, BTDB_FREELIST_OFFSET, 1);
  idxlock.lockw();

  int found = 0;

  off_t saveoffset = BTDB_FREELIST_OFFSET;

  off_t offset = dbreadptr(saveoffset);
  if(errcode_ != ERRBTDBNOERR)
    goto dberr;

  while(offset != 0) {

    off_t nextoffset = dbreadidx(offset);
    if(errcode_ != ERRBTDBNOERR)
      goto dberr;

    if(nextoffset == offset) {

#ifndef NDEBUG
      cerr << "DEBUG: Index record at " << offset << " is corrupt!" << endl;
#endif

      errcode_ = ERRBTDBCORRUPT;
      goto dberr;
    }

    if((idxlen_ == idxlen) && (datlen_ = datlen)) {
      found = 1;
      break;
    }
    
    saveoffset = offset;
    offset = nextoffset;
  }

  if(found) {

    // We have found a free record with matching sizes.  The index record
    // had been read by dbreadidx above, which also sets ptrval_.  Also,
    // saveoffset now points to the chain pointer which pointed to this
    // empty record on the freelist.  We set this chain pointer to ptrval_,
    // which removes the empty record from the freelist.

    dbwriteptr(saveoffset, ptrval_);
    if(errcode_ != ERRBTDBNOERR)
      goto dberr;
  }

  idxlock.unlock();
  return found;

 dberr:
  idxlock.unlock();
  return 0;
}
