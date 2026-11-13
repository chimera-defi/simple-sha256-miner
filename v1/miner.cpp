#include <cstddef>
extern "C" unsigned char *SHA256(const unsigned char *, size_t, unsigned char *);
#include <atomic>
#include <thread>
#include <vector>
#include <iostream>
#include <iomanip>
#include <chrono>
#include <cstring>
#include <algorithm>
#include <unistd.h>
#include <cstdio>
#include <cstdlib>
#include <string>
std::atomic<uint64_t> next_chunk{0}, checked{0};
std::atomic<bool> found{false};
constexpr uint64_t CHUNK=65536;
constexpr uint64_t TARGET=0x81e9ec6acULL; // placeholder replaced below
int main(){
 const std::string prefix="gensys";
 const char* abc="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
 uint64_t target=0;
 for(char c:prefix){const char*p=strchr(abc,c);if(!p)return 2;target=(target<<6)|(p-abc);}
 unsigned hw=std::thread::hardware_concurrency();if(!hw)hw=2;
 unsigned workers=std::max(1u,std::min(8u,hw/2?hw/2:1));
 std::cout<<"START workers="<<workers<<" hardware_threads="<<hw<<" target_base64="<<prefix<<std::endl;
 auto start=std::chrono::steady_clock::now();
 std::vector<std::thread> pool;
 for(unsigned t=0;t<workers;t++)pool.emplace_back([&,t](){
  unsigned char digest[32];char msg[80];
  while(!found.load(std::memory_order_relaxed)){
   uint64_t base=next_chunk.fetch_add(CHUNK);
   for(uint64_t j=0;j<CHUNK && !found.load(std::memory_order_relaxed);++j){
    uint64_t nonce=base+j;
    int len=snprintf(msg,sizeof(msg),"gensys:%llu",(unsigned long long)nonce);
    SHA256((unsigned char*)msg,len,digest);
    uint64_t first36=((uint64_t)digest[0]<<28)|((uint64_t)digest[1]<<20)|((uint64_t)digest[2]<<12)|((uint64_t)digest[3]<<4)|(digest[4]>>4);
    if(first36==target){
      if(!found.exchange(true)){
       std::cout<<"FOUND nonce="<<nonce<<" input="<<msg<<" hex=";
       for(unsigned char b:digest)std::cout<<std::hex<<std::setfill('0')<<std::setw(2)<<(unsigned)b;
       std::cout<<std::dec<<" base64=";
       for(int i=0;i<30;i+=3){unsigned x=(digest[i]<<16)|(digest[i+1]<<8)|digest[i+2];for(int k=18;k>=0;k-=6)std::cout<<abc[(x>>k)&63];}
       unsigned x=(digest[30]<<16)|(digest[31]<<8);std::cout<<abc[(x>>18)&63]<<abc[(x>>12)&63]<<abc[(x>>6)&63]<<"="<<std::endl;
      }
      break;
    }
   }
   checked.fetch_add(CHUNK,std::memory_order_relaxed);
  }
 });
 while(!found.load()){
  std::this_thread::sleep_for(std::chrono::seconds(10));
  double s=std::chrono::duration<double>(std::chrono::steady_clock::now()-start).count();
  std::cout<<"PROGRESS checked="<<checked.load()<<" elapsed_s="<<(uint64_t)s<<" avg_MH_s="<<std::fixed<<std::setprecision(2)<<(checked.load()/s/1e6)<<std::endl;
 }
 for(auto &th:pool)th.join();
 std::cout<<"DONE checked_approx="<<checked.load()<<std::endl;
}
