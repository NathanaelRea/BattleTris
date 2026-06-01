/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTPimp.C                                            */
/*    DATE: Fri Apr 29 02:27:24 1994                            */
/****************************************************************/

#include "BTConfig.H"


#include <iostream>
using namespace std;
#include <stdio.h>

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include "BTPimp.H"
#include "BTDirs.H"

static int max_weapons = BT_MAX_WEAPONS;

BTPimp::BTPimp() {
  for(int i = 0; i < BT_MAX_WEAPONS; i++) {
    cathouse_[i] = new BTWeapon((BTWeaponToken) i);
    cathouse_[i]->duration_ = 3;
    cathouse_[i]->token_ = (BTWeaponToken) i;
  }
}

BTWeapon *BTPimp::operator[] (BTWeaponToken index) {
  return cathouse_[(int) index];
}

BTWeapon *BTPimp::operator[] (int index) {
  return cathouse_[index];
}

void BTPimp::purchase (BTWeaponToken index) {
  purchases_[(int) index]++; 
}

#define READLINE(buf, file) \
  do { \
    if (fgets((buf), sizeof ((buf)), (file)) == NULL) \
      goto out; \
  } while ((buf)[0] == '#');

//
// Has worse code ever been written for a simpler operation?
//
int BTPimp::load()
{
  char buffer1[1024];
  char buffer2[4096];
  char buffer3[1024];
  char buffer4[1024];
  int i = 0;

  FILE *file,*file2;

  if(!(file = fopen(BTDB_WEAPONS,"r")))
    return 0;

  if(!(file2 = fopen(BTDB_WEAPONSP,"r")))
    return 0;

  while( i < max_weapons ) {
    READLINE(buffer1, file);
    READLINE(buffer2, file);
    READLINE(buffer3, file2);
    READLINE(buffer4, file2);

    if(!(buffer1[0] | buffer2[0] | buffer3[0] | buffer4[0]))
      return 0;

    buffer1[strlen(buffer1)-1] = 0;
    buffer2[strlen(buffer2)-1] = 0;
    buffer3[strlen(buffer3)-1] = 0;
    buffer4[strlen(buffer4)-1] = 0;

    delete cathouse_[i];
    cathouse_[i] = new BTWeapon(i, buffer1, buffer2, atoi(buffer3), atoi(buffer4));
    i++;
    READLINE(buffer4, file2);
  }

out:
  fclose(file);
  fclose(file2);

  //
  // If we failed to read in all weapons, then we need to return failure.
  //
  if (i < max_weapons) {
    cerr << "BattleTris: Bad weapons database: "
         << "found " << i << " weapons; expected " << max_weapons
         << endl;
    return 0;
  }

  return 1;
}

BTPimp::~BTPimp()
{
  for ( int i = 0 ; i < BT_MAX_WEAPONS ; i ++ )
    delete cathouse_[i];
}
