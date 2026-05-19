/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: DevAudio.C                                          */
/*    DATE: Thu Feb 10 14:52:44 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if HAVE_SUNAUDIO
/*
 * It's a goddamned crime that this isn't defined anywhere...
 */
typedef struct {
	uint32_t	audio_magic;
	uint32_t	audio_hdr_size;
	uint32_t	audio_data_size;
	uint32_t	audio_encoding;
	uint32_t	audio_sample_rate;
	uint32_t	audio_channels;
} audio_hdr_t;

/*
 * Can you fucking believe this shit?  Do we hate our customers?
 */
#define AUDIO_FILE_MAGIC		((uint32_t)0x2e736e64)
#define AUDIO_FILE_ENCODING_MULAW_8	(1)	/* 8-bit ISDN u-law */

#include <sys/audioio.h>
#endif

#include <sys/stat.h>


#include <iostream>
using namespace std;
#include <stdio.h>
#include <fcntl.h>
#include <errno.h>

#if STDC_HEADERS
# include <stdlib.h>
#endif

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#include "SigReceiver.H"
#include "DevAudio.H"

const char *DEVAUDIO_PATH = "/dev/audio";
const unsigned int DEVAUDIO_TIMEOUT = 15;

static int audio_fd = -1;

class DevAudioHandler : public SigHandler {
protected:
  int master_fd_, audio_fd_;
public:
  DevAudioHandler(int master_fd, int audio_fd,
                  SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp), master_fd_(master_fd), audio_fd_(audio_fd) {}
  virtual ~DevAudioHandler() {}
  void handle();
};

void DevAudioHandler::handle()
{
  close(master_fd_);
  close(audio_fd_);
  exit(0);
}

class DevAudioTimer : public SigHandler {
public:
  DevAudioTimer(SigDisposition disp = SIG_ENABLED) : SigHandler(disp) {}
  virtual ~DevAudioTimer() {}
  void handle();
};

void DevAudioTimer::handle()
{
  // Don't do anything here -- just want to cause blocking read to fail
  // in DevAudio::SlaveLoop indicating that timer has expired
}

DevAudio::DevAudio(int headphones, const char *root)
: master_fd_(-1), fd_(-1), slave_fd_(-1), valid_(0)
{  
	struct stat st;
	int pfd[2];	

	if (root == 0) 
		root_[0] = 0;
	else
		strcpy(root_, root);

#if HAVE_SUNAUDIO 
	if (stat(DEVAUDIO_PATH, &st) < 0) {
		cerr << "BattleTris: Failed to stat audio device" << endl;
		return;
	}
 
	if ((fd_ = open(DEVAUDIO_PATH, O_WRONLY)) == -1) {
		cerr << "BattleTris: Failed to open audio device" << endl;
		return;
	}

	close(fd_);

	/*
	 * Need to open a pipe, and fork() a slave
	 */
	if (pipe(pfd) < 0) {
		cerr << "BattleTris: Failed to open audio pipe" << endl;
		return;
	}   

	if ((slave_pid_ = fork()) < 0) {
		cerr << "BattleTris: Failed to fork audio slave" << endl;
		return;
	}

	/*
	 * If fork succeeded, then we're alive enough to indicate that
	 * BTStartup should not delete us
	 */
  	valid_ = 1;

	if (slave_pid_ == 0) {
		if ((fd_ = open(DEVAUDIO_PATH, O_WRONLY | O_NDELAY)) == -1)
			fd_ = open ("/dev/null", O_WRONLY);

		if (headphones) {
			audio_info info;

			cout << "BattleTris: Headphone sound enabled" << endl;
			AUDIO_INITINFO(&info);
			info.play.port = 0;
			info.play.port |= AUDIO_HEADPHONE;

			info.play.encoding = AUDIO_ENCODING_ULAW;
			info.play.precision = 8;
			info.play.channels = 1;
			info.play.sample_rate = 8000;

			ioctl(audio_fd, AUDIO_SETINFO, &info);
		} else {
			audio_info info;

			AUDIO_INITINFO(&info);
			info.play.port = 0;
			info.play.port |= AUDIO_SPEAKER;
			ioctl(audio_fd, AUDIO_SETINFO, &info);
		}
      
		close(pfd[1]);
		master_fd_ = pfd[0];

		SlaveLoop();
	}

	close(pfd[0]);
	slave_fd_ = pfd[1];
#else
	valid_ = 1;
#endif
}

void
DevAudio::QueueFile(const char *filename)
{
#if HAVE_SUNAUDIO
	char fullpath[1024];

	strcpy(fullpath, root_);
	strcat(fullpath, filename);

	/*
	 * All we need to do is send this down the pipe (just return if
	 * there isn't one).
	 */
	if(slave_fd_ == -1)
		return;

	/*
	 * Send the path plus the null
	 */
	write(slave_fd_, fullpath, strlen(fullpath) + 1);
#endif
}

void
DevAudio::SlaveLoop()
{
#if HAVE_SUNAUDIO
	DevAudioHandler handler(master_fd_, fd_);
	DevAudioTimer timer;

	SigReceiver sigrec;
	pid_t ppid = getppid();

	char filename[1024];
	int i;

	sigrec.reset();

	sigrec.install(SIGINT, &handler);
	sigrec.install(SIGHUP, &handler);
	sigrec.install(SIGTERM, &handler);
	sigrec.install(SIGPIPE, &handler);
	sigrec.install(SIGALRM, &timer);

	for (;;) {
		i = 0;

		do {
			alarm(DEVAUDIO_TIMEOUT);

			while (read(master_fd_, &filename[i], 1) != 1) {
				if (kill(ppid, 0) < 0 && errno == ESRCH) {
					cerr << "BattleTris: Sound daemon "
					    "aborting because game has died\n";
					close(master_fd_);
					close(fd_);
					exit(0);
				} else {
					alarm(0); // Reset the alarm timer
					alarm(DEVAUDIO_TIMEOUT);
        			}
			}

			alarm(0); // Clear the alarm timer

		} while(filename[i++] != 0);

		DumpFile(&filename[0]);
	}
#endif
}

#if HAVE_SUNAUDIO
static uint32_t
au2native(uint32_t val)
{
#if defined(sparc)
	return (val);
#elif defined(i386)
	uint32_t ret;

	((char *)&ret)[0] = ((char *)&val)[3];
	((char *)&ret)[1] = ((char *)&val)[2];
	((char *)&ret)[2] = ((char *)&val)[1];
	((char *)&ret)[3] = ((char *)&val)[0];

	return (ret);
#else
#error Unrecognized platform.
#endif
}
#endif /* HAVE_SUNAUDIO */
void
DevAudio::DumpFile(const char *file)
{
#if HAVE_SUNAUDIO
	int fd;
	audio_hdr_t hdr;
	uint32_t *i;
	ssize_t cnt, err;
	char buf[64 * 1024];

	if ((fd = open(file, O_RDONLY)) == -1) {
		cerr << "BattleTris: Failed to open sound file '" <<
		    file << "'" << endl;
		return;
	}

	if (read(fd, &hdr, sizeof (hdr)) != sizeof (hdr)) {
		cerr << "BattleTris: couldn't read header for sound file '" <<
		    file << "'" << endl;
		return;
	}

	for (i = (uint32_t *)&hdr;
	    (uintptr_t)i - (uintptr_t)&hdr < sizeof (hdr); i++)
		*i = au2native(*i);

	if (hdr.audio_magic != AUDIO_FILE_MAGIC) {
		cerr << file << " is not an audio file (bad magic)" << endl;
		return;
	}

	if (hdr.audio_encoding != AUDIO_FILE_ENCODING_MULAW_8) {
		cerr << file << " has unsupported audio encoding " <<
		    hdr.audio_encoding << endl;
		return;
	}

	lseek(fd, hdr.audio_hdr_size, SEEK_SET);

	ioctl(fd_, AUDIO_DRAIN, 0);

	while ((cnt = read(fd, buf, sizeof (buf))) >= 0) {
		while (((err = write(fd_, buf, cnt)) == -1) && errno == EAGAIN)
			continue;
		
		if (err != cnt || cnt == 0)
			break;
	}

	close(fd);
#endif
}    

DevAudio::~DevAudio()
{
#if HAVE_SUNAUDIO
	if (valid_) {
		kill(slave_pid_, SIGTERM);
		close(master_fd_);
		close(slave_fd_);
		close(fd_);
	}
#endif
}
