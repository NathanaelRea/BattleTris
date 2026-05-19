#ifndef LISTITER_H
#define LISTITER_H

#include "BTConfig.H"

#include "AbsListIter.H"
#include "ListElement.H"

//  class ListIter
//
//  This may well be the only documented use of protected inheritence.
//  The reason to use protected inheritence (in lieu of private) is 
//  as follows:  a ListIter is implemented in terms of an AbsListIter,
//  but we don't want people to be able to implictly upcast.  So far,
//  private inheritence would be fine.  The problem is that an RWListIter
//  needs to inherit from a ListIter...it _too_ is implemented in terms
//  of an AbsListIter.  Our choices are to have ListIter inherit privately
//  and then have RWListIter multiply inherit publicly from ListIter and
//  privately from AbsListIter.  Besides the fact that it creates general
//  nastiness, it also is a hack to simply re-create what the language
//  already has:  protected inheritence  (but you have my personal 
//  guarantee that you'll never see this anywhere else).
//

template<class T>
class ListIter : protected AbsListIter {
friend class List<T>;

private:
  ListElement<T> *peek_next_() { 
    return (ListElement<T> *) AbsListIter::peek_next();
  }
  ListElement<T> *peek_prev_() { 
    return (ListElement<T> *) AbsListIter::peek_prev();
  }
public:
  ListIter (List<T>& list) : AbsListIter (&list) { list.add_iter_ (this);  }
  ~ListIter() { ((List<T> *) this->list_)->remove_iter_ (this); }
  void jump_before_head() { AbsListIter::jump_before_head(); }
  void jump_after_tail() { AbsListIter::jump_after_tail(); }
  inline int peek_next(T& data);
  inline int peek_prev(T& data); 
  inline int inc();
  inline int dec();
  ListIter<T>& operator++() { AbsListIter::inc(); return *this; }
  ListIter<T>& operator--() { AbsListIter::dec(); return *this; }
};

template<class T> int ListIter<T>::peek_next(T& data) {
  ListElement<T> *elem = (ListElement<T> *) AbsListIter::peek_next(); 
  if (!elem)  
    return (0);
  data = elem->data();
  return(1);
}

template<class T> int ListIter<T>::peek_prev(T& data) {
  ListElement<T> *elem = (ListElement<T> *) AbsListIter::peek_prev(); 
  if (!elem)  
    return (0);
  data = elem->data();
  return(1);
}

template<class T> int ListIter<T>::inc() {
  ListElement<T> *elem = (ListElement<T> *) AbsListIter::peek_next(); 
  if (!elem)  
    return (0);
  AbsListIter::inc();
  return(1);
}

template<class T> int ListIter<T>::dec() {
  ListElement<T> *elem = (ListElement<T> *) AbsListIter::peek_prev(); 
  if (!elem)  
    return (0);
  AbsListIter::dec();
  return(1);
}
   
#endif
