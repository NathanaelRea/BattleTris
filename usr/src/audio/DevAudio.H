/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: DevAudio.H                                          */
/*    ASSN:                                                     */
/*    DATE: Thu Feb 10 14:48:18 1994                            */
/****************************************************************/

#ifndef DEVAUDIO_H
#define DEVAUDIO_H

#include "BTConfig.H"

#include <sys/types.h>

class DevAudio {
private:
	int fd_, slave_fd_, master_fd_;
	pid_t slave_pid_;
	char root_[1024];
	int valid_;

public:
	DevAudio(int headphones = 0, const char *root = 0);
	~DevAudio();

	void QueueFile(const char *filename);
	void DumpFile(const char *filename);
	void SlaveLoop();
	pid_t slavePid() { return slave_pid_; }

	void operator<< (const char *f) { QueueFile (f); }
	operator int() { return valid_; }
};

#endif
