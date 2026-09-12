from tools.references import reference


def built(source: bytes, assembled: bytes) -> bytes:
    return reference.built(source, assembled, "af15b6dc1332353f4bb35130f55b9de9c1b23aa33b71ebc53009a92446c22aa5")


if __name__ == "__main__":
    reference.main()
