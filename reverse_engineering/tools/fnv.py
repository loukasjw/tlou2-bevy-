def fnv(s,h=0xcbf29ce484222325):
    for c in s.encode(): h=((h^c)*0x100000001b3)&0xffffffffffffffff
    return h
if __name__=="__main__":
    print("%016x"%fnv("dc1/ss-medicine/ss-med-crossroad-floor-settling-igc.bin"), "want 0004d9b4289bb095")
    print("%016x"%fnv("dc1/weapon-damages.bin"))
