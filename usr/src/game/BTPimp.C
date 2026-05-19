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

int BTPimp::load() 
{
  char buffer1[1024];
  char buffer2[4096];
  char buffer3[1024];
  char buffer4[1024];

  FILE *file,*file2;

  if(!(file = fopen(BTDB_WEAPONS,"r")))
    return 0;

  if(!(file2 = fopen(BTDB_WEAPONSP,"r")))
    return 0;
   
  int i = 0;

  while( i < max_weapons ) {
    do {fgets(buffer1,sizeof(buffer1),file);} while ( buffer1[0]=='#' );
    do {fgets(buffer2,sizeof(buffer2),file);} while ( buffer2[0]=='#' );
    do {fgets(buffer3,sizeof(buffer3),file2);} while ( buffer3[0]=='#' );
    do {fgets(buffer4,sizeof(buffer4),file2);} while ( buffer4[0]=='#' );

    if(!(buffer1[0] | buffer2[0] | buffer3[0] | buffer4[0]))
      return 0;

    buffer1[strlen(buffer1)-1] = 0;
    buffer2[strlen(buffer2)-1] = 0;
    buffer3[strlen(buffer3)-1] = 0;
    buffer4[strlen(buffer4)-1] = 0;

    delete cathouse_[i];
    cathouse_[i] = new BTWeapon(i, buffer1, buffer2, atoi(buffer3), atoi(buffer4));
    i++;
    do {fgets(buffer4,sizeof(buffer4),file2);} while ( buffer4[0]=='#' );
  }

  fclose(file);
  fclose(file2);

  return 1;
}

BTPimp::~BTPimp()
{
  for ( int i = 0 ; i < BT_MAX_WEAPONS ; i ++ )
    delete cathouse_[i];
}
