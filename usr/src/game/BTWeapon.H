/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTWeapon.H                                          */
/*    ASSN:                                                     */
/*    DATE: Mon Feb 21 15:50:50 1994                            */
/****************************************************************/

#ifndef BTWEAPON_H
#define BTWEAPON_H

#include "BTConfig.H"
#include "BTProtocol.H"
#include "Block.H"

#include <string.h>

class BTWeapon {

friend class BTCommManager;
friend class BTComputer;
friend class BTBazaar;
friend class BTPimp;
friend class BTArsenal;
friend class BTGame;
friend class BTSoundManager;

public:
  BTWeaponToken token_;
  unsigned short duration_;
  unsigned short price_;
  char *name_;
  char *description_;
  
public:
  BTWeapon(BTWeaponToken token)
  : token_(token), duration_(0), price_(0), name_(0), description_(0) {}

  BTWeapon(int token, char * const name, char * const descr,
           unsigned short price, unsigned short duration)
  : token_((BTWeaponToken) token), duration_(duration), price_(price) {
    name_ = new char [strlen(name) + 1];
    strcpy(name_, name);
    description_ = new char [strlen(descr) + 1];
    strcpy(description_, descr);
  }

  ~BTWeapon() {
    delete name_;
    delete description_;
  }
  BTWeaponToken token() { return token_; }
  unsigned short duration() { return duration_; }
  unsigned short price() { return price_; }
};
     
#endif
