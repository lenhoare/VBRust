"""`FileSystem` — the VBR standard library's file I/O, on Python's own `open`/
`os`/`shutil`. Every fallible call returns `Ok`/`Err`, mirroring the Rust
`vbr_stdlib::FileSystem` surface (`Result<_, String>`)."""

import os
import shutil

from .prelude import Ok, Err


class FileSystem:
    @staticmethod
    def read(path):
        try:
            with open(path, "r") as f:
                return Ok(f.read())
        except OSError as e:
            return Err(str(e))

    @staticmethod
    def readlines(path):
        try:
            with open(path, "r") as f:
                return Ok([line.rstrip("\n") for line in f])
        except OSError as e:
            return Err(str(e))

    @staticmethod
    def write(path, contents):
        try:
            with open(path, "w") as f:
                f.write(contents)
            return Ok(None)
        except OSError as e:
            return Err(str(e))

    @staticmethod
    def append(path, text):
        try:
            with open(path, "a") as f:
                f.write(text)
            return Ok(None)
        except OSError as e:
            return Err(str(e))

    @staticmethod
    def exists(path):
        return os.path.isfile(path)

    @staticmethod
    def copy(source, destination):
        try:
            shutil.copyfile(source, destination)
            return Ok(None)
        except OSError as e:
            return Err(str(e))

    @staticmethod
    def movefile(source, destination):
        try:
            shutil.move(source, destination)
            return Ok(None)
        except OSError as e:
            return Err(str(e))

    @staticmethod
    def delete(path):
        try:
            os.remove(path)
            return Ok(None)
        except OSError as e:
            return Err(str(e))

    @staticmethod
    def createfolder(path):
        try:
            os.mkdir(path)
            return Ok(None)
        except OSError as e:
            return Err(str(e))

    @staticmethod
    def createfolderall(path):
        try:
            os.makedirs(path, exist_ok=True)
            return Ok(None)
        except OSError as e:
            return Err(str(e))

    @staticmethod
    def folderexists(path):
        return os.path.isdir(path)

    @staticmethod
    def deletefolder(path):
        try:
            os.rmdir(path)
            return Ok(None)
        except OSError as e:
            return Err(str(e))

    @staticmethod
    def deletefolderall(path):
        try:
            shutil.rmtree(path)
            return Ok(None)
        except OSError as e:
            return Err(str(e))
