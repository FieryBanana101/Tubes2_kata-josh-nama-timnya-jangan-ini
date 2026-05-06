## Tugas Besar 2 (IF2211) - Penerapan DFS/BFS Traversal pada HTML DOM Tree

### Kelompok 44 - K02:
<div align="center" id="contributor">
   <strong>
     <h3>~ Tim "kata josh nama timnya jangan ini"  ~</h3>
     <table align="center">
       <tr align="center">
         <td>NIM</td>
         <td>Nama</td>
       </tr>
       <tr align="center">
         <td>13524004</td>
         <td>Muhammad Fatih Irkham Mauludi </td>
       </tr>
       <tr align="center">
         <td>13524048</td>
         <td>Josh Reinhart Zidik </td>
       </tr>
       <tr align="center">
         <td>13524095</td>
         <td>Jingglang Galih Rinenggan</td>
       </tr>
     </table>
   </strong>
 </div>
 
 <div align="center">
   <h3 align="center"> Tech Stacks </h3>
   <p align="center">

![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)
![WebAsm](https://img.shields.io/badge/-WebAssembly-654FF0?style=flat-square&logo=webassembly&logoColor=white)

   </p>
 </div>



### Deskripsi

Proyek ini merupakan bentuk aplikasi algoritma traversal graf BFS dan DFS dalam pencarian interaktif pohon DOM HTML, dengan aplikasi berbasis web.

DFS dan BFS dilakukan secara multi-threading menggunakan struktur data global stack dan queue secara respektif,
setiap thread akan mendapatkan tugas dari struktur data global tersebut berdasarkan CSS Selector yang telah di-parsing.

Pengguna dapat mengubah HTML menjadi DOM Tree berdasarkan masukan teks langsung, melalui URL (web scraping) ataupun melalui file. Kemudian hasil DOM Tree tersebut dapat dicocokkan dengan sebuah CSS Selector dan dapat melihat hasil
pencarian Lowest Common Ancestor antara dua simpul atau lebih, secara interaktif.

Aplikasi web tersebut dapat diakses pada link berikut: https://html-dom-explorer.682b5f20.nip.io/

Terdapat contoh file html yang bisa digunakan untuk pengujian pada direktori "example/".

### Requirement dan Instalasi
Pertama buatlah sebuah file ".env" pada direktori paling atas dari repositori, file tersebut berisi format:
```bash
DOMAIN="<domain here>"
```

Masukkanlah domain yang ingin diberikan pada aplikasi web yang akan diluncurkan, 
jika hanya secara lokal maka dapat diisi dengan "localhost".


Instalasi yang direkomendasikan hanya membutuhkan [Docker](https://docs.docker.com/engine/install), 
silahkan ikuti panduan resmi untuk instalasi Docker (catatan: pastikan Docker mendukung fitur Docker Compose).

Kemudian instalasi dapat dilakukan dengan command berikut pada direktori paling atas dari repositori,
```bash
docker compose up --build 
```

Command tersebut akan langsung membuat service-service yang dibutuhkan dan dapat langsung diakses melalui
```bash
https://localhost:80
https://localhost:443
```

Tidak disarankan melakukan build manual melalui Cargo ataupun build system lainnya.