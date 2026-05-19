/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTDebug.H                                           */
/*    DATE: Wed Apr 13 16:48:20 1994                            */
/****************************************************************/

#ifndef BTDEBUG_H
#define BTDEBUG_H

#include "BTConfig.H"
#include "BTConstants.H"

#ifndef NDEBUG


#include <iostream>
using namespace std;

#define BTDebug(x)  cerr << BT_VERSION << " [" << __FILE__ << ", " << __LINE__ << "] DEBUG: " << x << endl

#else

#define BTDebug(x) 

#endif

#endif
