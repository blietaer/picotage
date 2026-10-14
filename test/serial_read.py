#!/usr/bin/env python3

# /////////////////////////////////////////////////////////////////////////////
#
#   (C) Copyright 2023, blietaer
#       blietaer@yahoo.com
#
# /////////////////////////////////////////////////////////////////////////////

# ////////////////////////////////////////////////////////////////////////////
# Imports LIBs
# ////////////////////////////////////////////////////////////////////////////

# /*  Global Includes */
import serial
import signal
import struct
import hexdump
import logging
import coloredlogs
from time import sleep
from logging.config import dictConfig

# /*  Project Specific */

# this port address is for the serial tx/rx pins on the GPIO header
SERIAL_PORT = '/dev/ttyUSB0'
# be sure to set this to the same rate used on the Arduino
SERIAL_RATE = 115200
DFLT_LOGGNG_FILE = "/opt/smart_meter/log/smart_meter.log"
RUNNING = True
DEFAULT_LOG_FORMAT = (
            "[%(asctime)s] [%(levelname)-8s] [%(name)s:%(lineno)-4d] :: %(message)s"
            )

#log_level = logging.INFO
log_level = logging.DEBUG


logging_config = dict(
        version=1,
        formatters={
            "f": {
                "format": "[%(asctime)s] [%(levelname)-8s] [%(name)s:%(lineno)d] :: %(message)s"
                }
            },
        handlers={
            "console": {
                "class": "logging.StreamHandler",
                "formatter": "f",
                "level": log_level,
                },
            "file": {
                "class": "logging.FileHandler",
                "filename": DFLT_LOGGNG_FILE,
                "formatter": "f",
                "level": log_level,
                },
            },
        root={"handlers": ["console", "file"], "level": log_level},
        )


dictConfig(logging_config)
logger = logging.getLogger(__name__)
coloredlogs.install(fmt=DEFAULT_LOG_FORMAT, level=log_level)


# CTRL-C Signal registration
####################################
def signal_handler(signal, frame):
    RUNNING = False
    logger.warning("User requested CTRL+C")
    raise KeyboardInterrupt


# ////////////////////////////////////////////////////////////////////////////
#  Class RunMe
# ////////////////////////////////////////////////////////////////////////////
class RunMe(object):
        
    def open(self):
        self.ser = serial.Serial(
            port=SERIAL_PORT,\
            baudrate=SERIAL_RATE,\
            parity=serial.PARITY_NONE,\
            #        stopbits=serial.STOPBITS_ONE,\
            #        bytesize=serial.EIGHTBITS,\
            timeout=3)
        logger.info("Port [%s] is open: %s" %(SERIAL_PORT, self.ser.isOpen() ) )
        self.clear()

    def stop_read_loop(self):
        logger.info("Stopping the reading loop ! ")
        RUNNING = False
        self.clear()

    def clear(self):
        self.ser.flushOutput()
        self.ser.flushInput()

    def close(self):
        self.clear()
        self.ser.close()
        logger.info("Port [%s] is open: %s" %(SERIAL_PORT, self.ser.isOpen() ) )

    def start_read_loop(self):
        while(RUNNING):
            try:
                print(self.ser.readline().decode('utf-8').strip('\n') )
            except Exception as e:
                logger.warning(f"ERROR Reading: {e}")
                self.clear()
            except KeyboardInterrupt:
                logger.warning("KeyBoard Interrupt occured ! ")
                self.stop_read_loop()
                break
            finally:
                if not RUNNING:
                    break


# ////////////////////////////////////////////////////////////////////////////
#  main()
# ////////////////////////////////////////////////////////////////////////////
def main() -> None:
    """Main entry point for the script."""
    signal.signal(signal.SIGINT, signal_handler)
    smart_meter = RunMe()
    smart_meter.open()
    smart_meter.start_read_loop()
    smart_meter.close()
    exit()

# ////////////////////////////////////////////////////////////////////////////
#  main() trick
# ////////////////////////////////////////////////////////////////////////////
if __name__ == "__main__":
    main()
