"""Experimental json.dump fast path; application imports/calls remain unchanged.

This is an explicit runtime bootstrap patch, not a modification to the installed
CPython, a new JSON encoder, or a claim of complete Python compatibility.
"""

import functools
import io
import json
import os
import stat

import _pynox_json

_dump = json.dump
_dumps = json.dumps
_encoder = json._default_encoder
_encoder_state = vars(_encoder).copy()
_encoder_class = json.JSONEncoder
_encode = _encoder_class.encode
_iterencode = _encoder_class.iterencode
_default = _encoder_class.default
_make_encoder = json.encoder.c_make_encoder
_ascii_encoder = json.encoder.encode_basestring_ascii
_plain_encoder = json.encoder.encode_basestring


def _ordinary_file(fp):
    # Custom writers, pipes, binary/update/unbuffered streams keep streaming dump.
    if type(fp) is not io.TextIOWrapper:
        return False
    if fp.closed or fp.line_buffering or fp.write_through:
        return False
    if type(fp.buffer) is not io.BufferedWriter or type(fp.buffer.raw) is not io.FileIO:
        return False
    if fp.encoding.lower().replace("_", "-") not in ("utf-8", "utf8", "ascii", "us-ascii"):
        return False
    if set(vars(fp)) - {"mode"} or vars(fp.buffer) or set(vars(fp.buffer.raw)) - {"name"}:
        return False
    return stat.S_ISREG(os.fstat(fp.fileno()).st_mode)


def _ordinary_encoder():
    return (
        json._default_encoder is _encoder
        and json.JSONEncoder is _encoder_class
        and vars(_encoder) == _encoder_state
        and _encoder_class.encode is _encode
        and _encoder_class.iterencode is _iterencode
        and _encoder_class.default is _default
        and json.encoder.c_make_encoder is _make_encoder
        and json.encoder.encode_basestring_ascii is _ascii_encoder
        and json.encoder.encode_basestring is _plain_encoder
    )


@functools.wraps(_dump)
def dump(
    obj,
    fp,
    *,
    skipkeys=False,
    ensure_ascii=True,
    check_circular=True,
    allow_nan=True,
    cls=None,
    indent=None,
    separators=None,
    default=None,
    sort_keys=False,
    **kw,
):
    ordinary = (
        skipkeys is False
        and ensure_ascii is True
        and check_circular is True
        and allow_nan is True
        and cls is None
        and indent is None
        and separators is None
        and default is None
        and sort_keys is False
        and not kw
    )
    if ordinary and _ordinary_encoder():
        try:
            regular_file = _ordinary_file(fp)
        except (OSError, ValueError):
            regular_file = False
        if regular_file and _pynox_json.try_dump(obj, fp, _dumps):
            return None
    _pynox_json.fallback()
    # Keep the ordinary streaming algorithm in this frame. Calling _dump from
    # another Python frame would change recursion-limit failures/partial output.
    cached = (
        not skipkeys
        and ensure_ascii
        and check_circular
        and allow_nan
        and cls is None
        and indent is None
        and separators is None
        and default is None
        and not sort_keys
        and not kw
    )
    if cached:
        encoder = json._default_encoder
    else:
        factory = json.JSONEncoder if cls is None else cls
        encoder = factory(
            skipkeys=skipkeys,
            ensure_ascii=ensure_ascii,
            check_circular=check_circular,
            allow_nan=allow_nan,
            indent=indent,
            separators=separators,
            default=default,
            sort_keys=sort_keys,
            **kw,
        )
    for part in encoder.iterencode(obj):
        fp.write(part)


def install():
    json.dump = dump
