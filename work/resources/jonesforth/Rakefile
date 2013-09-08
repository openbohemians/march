
desc "compile jones forth"
task :build do
  sh "gcc -m32 -nostdlib -static -Wl,-Ttext,0 -Wl,--build-id=none -o jonesforth jonesforth.S"
end

desc "run jones forth?"
task :run do
  sh "cat jonesforth.f - | ./jonesforth"
end

