/****************************************************************/
/*    NAME: Charles G. Hoecker                                  */
/*    ACCT: cgh                                                 */
/*    FILE: BTStack.C                                           */
/*    ASGN: Final                                               */
/*    DATE: Mon Oct  3 16:01:59 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <string.h>

#include "BTStack.H"

#define RESIZE_RATIO 2

template<class T>
void BTStack<T>::tailInsert( const T& data )
{
  if ( mem_ == NULL ) {
    mem_tail_ = mem_ = (char *) malloc( mem_no_ * sizeof(T) );
    mem_end_ = mem_ + (mem_no_ * sizeof(T));
  }
  if ( mem_tail_ == mem_end_ ) {
    char *new_mem;
    new_mem = (char *)malloc( mem_no_ * RESIZE_RATIO * sizeof(T) );
    memcpy( new_mem, mem_, mem_no_ * sizeof(T) );
    mem_tail_ = mem_tail_ - mem_ + new_mem;
    mem_no_ *= RESIZE_RATIO;
    free( mem_ );
    mem_ = new_mem;
    mem_end_ = mem_ + mem_no_ * sizeof(T);
  }
//  memcpy( mem_tail_, &data, sizeof(T) );
  T *new_move = (T *) mem_tail_;
//  new_move->T();
  *new_move = data;
  mem_tail_ += sizeof(T);
  count_++;
}

template<class T>
void BTStack<T>::clear()
{
  mem_ptr_ = mem_tail_ = mem_;
  count_ = 0;
}

template<class T>
int BTStack<T>::getNext(T& data)
{
  if ( mem_ptr_ == mem_tail_ )
    return 0;
  T *da_move = (T *) mem_ptr_;
  data = *da_move;
  mem_ptr_ += sizeof(T);
  return 1;
}

template<class T>
int BTStack<T>::getPrev(T& data)
{
  if ( mem_ptr_ == mem_ )
    return 0;
  mem_ptr_ -= sizeof(T);
  T *da_move = (T *) mem_ptr_;
  data = *da_move;
  return 1;
}

template<class T>
int BTStack<T>::tailRemove(T& data)
{
  if ( mem_tail_ == mem_ )
    return 0;
  if ( mem_ptr_ == mem_tail_ )
    mem_ptr_ -= sizeof(T);
  mem_tail_ -= sizeof(T);
  count_--;
  data = *((T*)mem_tail_);
  return 1;
}

template<class T>
void BTStack<T>::operator=(BTStack<T> &other)
{
  unsigned long other_size = other.mem_tail_ - other.mem_;
  other_size /= sizeof(T);

  unsigned long new_size = mem_no_;
  while ( other_size > new_size )
    new_size *= RESIZE_RATIO;

  if (new_size > mem_no_) {
    void *new_mem;
    new_mem = malloc(new_size * sizeof(T));
    free(mem_);
    mem_ = (char *)new_mem;
    mem_no_ = new_size;
    mem_end_ = mem_ + mem_no_ * sizeof(T);
  }
  memcpy(mem_,other.mem_,other.mem_tail_-other.mem_);
  mem_tail_ = mem_ + (other.mem_tail_ - other.mem_);
  mem_ptr_ = mem_;
  count_ = ( mem_tail_ - mem_ ) / sizeof(T);
}
