# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

# pyre-strict

import argparse
import asyncio
import json
import os
import signal
import sys
from enum import Enum
from time import sleep
from typing import List, Optional

from .idb_companion import IdbCompanion

from .ios import ios_booted_simulator, ios_unbooted_simulator, prepare_simulator

idb_companions: List[IdbCompanion] = []


def _args_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Utility which helps to set up IDB companions which are used later by buck2 it runs tests locally."
    )
    parser.add_argument(
        "--simulator-manager",
        required=False,
        type=str,
        help="Tool to manage simulators and their lifecycle. Required for iOS testing",
    )
    parser.add_argument(
        "--type",
        metavar="<TYPE>",
        type=_ResourceType,
        choices=[e.value for e in _ResourceType],
        required=True,
        help=f"""
            Type of required resources.
            Pass `{_ResourceType.iosUnbootedSimulator}` to get a companion for iOS unbooted simulator.
            Pass `{_ResourceType.iosBootedSimulator}` to get a companion for iOS booted simulator.
        """,
    )
    parser.add_argument(
        "--no-companion",
        default=False,
        action="store_true",
        help="""
        If passed, will only create simulator. No idb_companion will be spawned.
        """,
    )
    return parser


class _ResourceType(str, Enum):
    iosUnbootedSimulator = "ios_unbooted_simulator"
    iosBootedSimulator = "ios_booted_simulator"


def _exit_gracefully(*args: List[object]) -> None:
    for idb_companion in idb_companions:
        idb_companion.cleanup()
    exit(0)


def _check_simulator_manager_exists(simulator_manager: Optional[str]) -> None:
    if not simulator_manager:
        raise Exception("Simulator manager is not specified")


def main() -> None:
    args = _args_parser().parse_args()
    if args.no_companion:
        booted = args.type == _ResourceType.iosBootedSimulator
        sim = asyncio.run(
            prepare_simulator(simulator_manager=args.simulator_manager, booted=booted)
        )
        result = {
            "resources": [
                {
                    "udid": sim.udid,
                    "device_set_path": sim.device_set_path,
                }
            ]
        }
        json.dump(result, sys.stdout)
    else:
        _create_companion(args)


def _create_companion(args: argparse.Namespace) -> None:
    if args.type == _ResourceType.iosBootedSimulator:
        _check_simulator_manager_exists(args.simulator_manager)
        idb_companions.extend(asyncio.run(ios_booted_simulator(args.simulator_manager)))
    elif args.type == _ResourceType.iosUnbootedSimulator:
        _check_simulator_manager_exists(args.simulator_manager)
        idb_companions.extend(
            asyncio.run(ios_unbooted_simulator(args.simulator_manager))
        )
    pid = os.fork()
    if pid == 0:
        # child
        signal.signal(signal.SIGINT, _exit_gracefully)
        signal.signal(signal.SIGTERM, _exit_gracefully)
        while True:
            sleep(0.1)
    else:
        # Do not leak open FDs in parent
        for c in idb_companions:
            c.stderr.close()
        result = {
            "pid": pid,
            "resources": [{"socket_address": c.socket_address} for c in idb_companions],
        }
        json.dump(result, sys.stdout)


if __name__ == "__main__":
    main()
