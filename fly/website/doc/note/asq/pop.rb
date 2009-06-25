
module Pudding

  COMMNADS = {}


  def evaluate

    $work_stack

  end

end


  def parse( inp )
    s = inp.scan(/.*?[.]/)  # TODO except in brackets and quotes
    l = s.collect { |e| e.strip.split(/[ ]+/) }
    p l
  end

  prompt = ":> "
  inp = ''
  while inp[-1,1] != '.'
    print prompt
    until inp << ' ' << $stdin.gets ; sleep 1 ; end
    inp.strip!
  end

  parse( inp )
