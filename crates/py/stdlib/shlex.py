import re


def quote(s):
    if not s:
        return "''"
    if re.fullmatch(r"[a-zA-Z0-9_@%+=:,./-]+", s):
        return s
    return "'" + s.replace("'", "'\"'\"'") + "'"


def join(tokens):
    return " ".join(quote(s) for s in tokens)


def split(s, comments=False, posix=True):
    if not posix:
        raise NotImplementedError("only POSIX shell syntax is supported")
    tokens, word, q, active, i = [], "", "", False, 0
    while i < len(s):
        c = s[i]
        i += 1
        if not q and c in " \r\n\t":
            if active:
                tokens.append(word)
            word, active = "", False
        elif not q and comments and c == "#":
            i = s.find("\n", i)
            if i < 0:
                break
        elif c in "'\"" and (not q or c == q):
            q, active = ("" if q else c), True
        elif c == "\\" and q != "'":
            if i == len(s):
                raise ValueError("No escaped character")
            if q and s[i] not in ('"', "\\"):
                word += c
            word += s[i]
            i, active = i + 1, True
        else:
            word += c
            active = True
    if q:
        raise ValueError("No closing quotation")
    if active:
        tokens.append(word)
    return tokens
