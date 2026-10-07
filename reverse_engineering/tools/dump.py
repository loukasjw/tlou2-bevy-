import struct,pickle,sys
H=pickle.load(open("dict.pkl","rb"))
B=r"C:\Users\louka\coding\ghidra tlou decoding\extracted\bin\dc1"+"\\"
def nm(h): return H.get(h,"?%016x"%h)
f=sys.argv[1]; d=open(B+f,"rb").read()
print(f,len(d))
magic,ver,sz1,sz2,a,cnt,tab,_=struct.unpack_from("<4sIIIIIII",d,0)
print("hdr",magic,ver,hex(sz1),hex(sz2),a,cnt,hex(tab))
print("file-hash",nm(struct.unpack_from("<Q",d,0x20)[0]))
for i in range(cnt):
    n,t,o=struct.unpack_from("<QQQ",d,0x28+i*24)
    print(i,nm(n),nm(t),hex(o))
# print all qwords that resolve
res={}
for o in range(0,len(d)-7,4):
    q=struct.unpack_from("<Q",d,o)[0]
    if q in H: res.setdefault(H[q],[]).append(o)
print(len(res),"distinct resolved names")
print(sorted(res)[:int(sys.argv[2]) if len(sys.argv)>2 else 80])
