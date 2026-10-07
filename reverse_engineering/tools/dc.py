import struct,pickle,sys
H=pickle.load(open("dict.pkl","rb"))
B=r"C:\Users\louka\coding\ghidra tlou decoding\extracted\bin\dc1"+"\\"
def N(q): return H.get(q,"?%016x"%q)
def show(d,p,depth=0,maxd=4):
    ind="  "*depth
    t=struct.unpack_from("<Q",d,p-8)[0]; tn=N(t)
    q=struct.unpack_from("<Q",d,p)[0]
    if tn=="map":
        cnt,kp,vp=struct.unpack_from("<QQQ",d,p)
        print(ind+"map[%d]"%cnt)
        if depth>=maxd: return
        for i in range(cnt):
            k=struct.unpack_from("<Q",d,kp+i*8)[0]
            v=struct.unpack_from("<Q",d,vp+i*8)[0]
            print(ind+" "+N(k)+":",end="")
            leaf(d,v,depth+1,maxd)
    else: leaf(d,p,depth,maxd,known=True)
def leaf(d,v,depth,maxd,known=False):
    ind="  "*depth
    if v<8 or v>=len(d): print(" imm",v); return
    t=struct.unpack_from("<Q",d,v-8)[0]; tn=N(t)
    if tn=="map": print(); show(d,v,depth,maxd)
    elif tn in("float","single"): print(" float",struct.unpack_from("<f",d,v)[0])
    elif tn in("int","integer"): print(" int",struct.unpack_from("<q",d,v)[0])
    elif tn=="symbol": print(" symbol",N(struct.unpack_from("<Q",d,v)[0]))
    elif tn=="boolean": print(" bool",struct.unpack_from("<Q",d,v)[0])
    elif tn=="string":
        e=d.index(b"\0",v); print(" str",d[v:e])
    else: print(" <%s> raw=%s"%(tn,d[v:v+32].hex()))
if __name__=="__main__":
    f,off=sys.argv[1],int(sys.argv[2],16);d=open(B+f,"rb").read();show(d,off,0,int(sys.argv[3]) if len(sys.argv)>3 else 3)
