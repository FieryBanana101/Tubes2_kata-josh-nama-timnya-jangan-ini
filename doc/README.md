### Recommended Workspace

Install texlive:
1. Install texlive from: https://tug.org/texlive/quickinstall.html
2. Or install texlive using package manager such as Ubuntu's `apt`, https://askubuntu.com/questions/1180776/install-latest-version-of-tex (not recommended due to outdated result)
3. Install all required package using `tgmr install <latex package names>` command
4. Build the document using `pdflatex` (please see next section)

Or you can just copy paste the *.tex file to an online Tex renderer such as https://www.overleaf.com/.

### How to build (using pdflatex)
Run the `./build.sh` for easy use, related build file will be in `build/` directory, the pdf document will appear.

And simply do this if you want to clean the build result,
```
rm -rf ./build/ *.pdf
```

Or you can run the `pdflatex` command by yourself to get the desired build settings.
