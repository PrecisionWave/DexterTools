#!/usr/bin/python3
import json
import sys
import zmq
from pprint import pprint
import argparse

context = zmq.Context()

sock = context.socket(zmq.REQ)

poller = zmq.Poller()
poller.register(sock, zmq.POLLIN)

sock.connect("tcp://127.0.0.1:5552")

# SetDesiredBank, Update and FormatOtherBank return either
#{'status': 'Ok', 'detail': "some string"}
# or
#{'status': 'Error', 'detail': "some string about the error"}

def send_command(command):
    sock.send(json.dumps(command).encode())

    socks = dict(poller.poll(8000))
    if socks:
        if socks.get(sock) == zmq.POLLIN:
            data = sock.recv()
            #print("Received: {}".format(data), file=sys.stderr)
            pprint(json.loads(data.decode()))

    else:
        print("ZMQ error: timeout", file=sys.stderr)
        context.destroy(linger=5)

# GetStatus returns the JSON that deserialises to:
# {'status': 'Status',
#  'progress': None
#  'banks': {
#   'our_bank': 'A',
#   'desired_bank': None,
#   'last_tried_bank': None,
#   'last_ok_bank': None,
#   'our_version': '2024-06-17 13:00:09+00:00',
#   'our_extract_time': None,
#   'other_version': '2024-06-17 13:00:09+00:00',
#   'other_extract_time': None}}
#
# progress is either None (JSON: null) when not updating, or a percentage when an update is ongoing
def do_get_status(cli_args):
    send_command({"command": "GetStatus"})


def do_update(cli_args):
    send_command({
        "command": "Update",
        "from_url": cli_args.url,
        "username": None,
        "password": None })

def do_set_desired_bank(cli_args):
    send_command({"command": "SetDesiredBank", "bank": cli_args.bank})

def do_set_bank_ok(cli_args):
    send_command({"command": "SetBankOk"})

parser = argparse.ArgumentParser(description="FW UPD TOOL remote control")
parser.set_defaults(func=lambda x: print("specify subcommand!"))
subparsers = parser.add_subparsers(help='Select among the following sub-commands:')

parser_get_status = subparsers.add_parser('get-status', help='Get FW upd bank info and status')
parser_get_status.set_defaults(func=do_get_status)

parser_update = subparsers.add_parser('update', help='Start a firmware update')
parser_update.add_argument('-u', '--url', required=True, help="URL from where to download the .tar.zstd")
parser_update.set_defaults(func=do_update)
# TODO --user and --pass optional arguments

parser_set_desired_bank = subparsers.add_parser('set-desired-bank', help='Set the bank from which to boot')
parser_set_desired_bank.add_argument('-b', '--bank', required=True, help="Bank. Possible values: A or B")
parser_set_desired_bank.set_defaults(func=do_set_desired_bank)

parser_set_ok_bank = subparsers.add_parser('set-bank-ok', help='Set the current bank as ok in the last_bank_ok variable')
parser_set_ok_bank.set_defaults(func=do_set_bank_ok)

cli_args = parser.parse_args()
cli_args.func(cli_args)
